//! Stato dell'app: cartella delle note, nota aperta, salvataggio e comandi.
//!
//! L'interfaccia è in `panels.rs` (le tre colonne) e `dialogs.rs` (finestre).

use crate::editor::{Editor, EditorAction, EditorCtx};
use crate::i18n::tr;
use crate::icons::{self, Icon};
use crate::settings::{Backdrop, Language, Settings, ThemeMode};
use crate::vault::{self, NoteMeta, Vault};
use crate::widgets::{self, pal};
use crate::{fsutil, theme};
mod dialogs;
mod panels;

use eframe::egui::{
    self, Align2, Color32, FontId, Id, Key, KeyboardShortcut, Modifiers, Pos2, Rect, RichText, Sense, ThemePreference, Vec2,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

const KOFI_URL: &str = "https://ko-fi.com/moosedevpjs";
const SAVE_DELAY: Duration = Duration::from_millis(500);
const RENAME_DELAY: Duration = Duration::from_millis(1000);
const RETRY_DELAY: Duration = Duration::from_secs(2);
const RESCAN_DELAY: Duration = Duration::from_millis(300);
const TOAST_TIME: Duration = Duration::from_secs(6);
/// Durata dell'animazione di entrata e uscita del messaggio.
const TOAST_ANIM: f32 = 0.22;

/// Quali note mostra l'elenco.
#[derive(Clone, PartialEq, Debug)]
enum Filter {
    All,
    Folder(PathBuf),
    Tag(String),
}

/// La nota aperta nell'editor.
struct OpenNote {
    /// `None` finché una nota nuova non viene salvata la prima volta.
    path: Option<PathBuf>,
    /// Cartella in cui creare il file se la nota è nuova.
    dir: PathBuf,
    text: String,
    saved: String,
    crlf: bool,
    bom: bool,
    /// Data di modifica del file quando l'abbiamo letto/scritto: serve a
    /// capire se qualcun altro lo ha cambiato nel frattempo.
    mtime: Option<SystemTime>,
    last_edit: Instant,
    edited: bool,
    last_failure: Option<Instant>,
    /// File aperto da fuori dalla cartella delle note (es. "Apri con"):
    /// si modifica quel file e basta, senza rinominarlo.
    external: bool,
}

impl OpenNote {
    fn dirty(&self) -> bool {
        self.text != self.saved
    }
}

/// Stato salvato nel vault, in `.yonote/state.toml`.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct VaultState {
    /// Percorsi relativi delle note fissate.
    pinned: Vec<String>,
    last_open: Option<String>,
}

impl VaultState {
    fn path(root: &Path) -> PathBuf {
        root.join(STATE_DIR).join("state.toml")
    }
    fn load(root: &Path) -> Self {
        std::fs::read_to_string(Self::path(root)).ok().and_then(|s| toml::from_str(&s).ok()).unwrap_or_default()
    }
    fn save(&self, root: &Path) {
        if let Ok(s) = toml::to_string_pretty(self) {
            let _ = fsutil::write_atomic(&Self::path(root), s.as_bytes());
        }
    }
}

/// Cartella dei dati dell'app dentro la cartella delle note.
const STATE_DIR: &str = ".yonote";

/// Le versioni chiamate "Blocks" usavano `.blocks`: la si rinomina una volta sola.
fn migrate_legacy_state_dir(root: &Path) {
    let (old, new) = (root.join(".blocks"), root.join(STATE_DIR));
    if old.is_dir()
        && !new.exists()
        && let Err(e) = std::fs::rename(&old, &new)
    {
        log_error(&format!("migrazione .blocks: {e}"));
    }
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

struct Toast {
    text: String,
    /// Note spostate nel Cestino che si possono ripristinare.
    undo: Vec<PathBuf>,
    created: Instant,
    until: Instant,
}

impl Toast {
    fn new(text: String, undo: Vec<PathBuf>, duration: Duration) -> Self {
        let now = Instant::now();
        Self { text, undo, created: now, until: now + duration }
    }
}

/// Eliminazione in attesa di conferma.
enum PendingDelete {
    Notes(Vec<PathBuf>),
    Folder(PathBuf),
}

/// Una voce della palette Ctrl+K.
#[derive(Clone)]
enum PaletteItem {
    Note(PathBuf, String),
    Create(String),
    Command(Command),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Command {
    NewNote,
    ToggleTheme,
    FocusMode,
    ToggleSidebar,
    Settings,
    PinNote,
    DeleteNote,
    RevealVault,
    ChangeVault,
}

impl Command {
    const ALL: [Command; 9] = [
        Command::NewNote,
        Command::ToggleTheme,
        Command::FocusMode,
        Command::ToggleSidebar,
        Command::Settings,
        Command::PinNote,
        Command::DeleteNote,
        Command::RevealVault,
        Command::ChangeVault,
    ];

    fn label(self) -> &'static str {
        match self {
            Command::NewNote => tr("Nuova nota", "New note"),
            Command::ToggleTheme => tr("Cambia tema chiaro/scuro", "Toggle light/dark theme"),
            Command::FocusMode => tr("Modalità concentrazione", "Focus mode"),
            Command::ToggleSidebar => tr("Mostra/nascondi cartelle", "Show/hide folders"),
            Command::Settings => tr("Impostazioni", "Settings"),
            Command::PinNote => tr("Fissa/sblocca la nota", "Pin/unpin note"),
            Command::DeleteNote => tr("Sposta la nota nel Cestino", "Move note to Recycle Bin"),
            Command::RevealVault => tr("Apri la cartella in Esplora file", "Open folder in File Explorer"),
            Command::ChangeVault => tr("Cambia cartella delle note…", "Change notes folder…"),
        }
    }

    fn shortcut(self) -> &'static str {
        match self {
            Command::NewNote => "Ctrl+N",
            Command::FocusMode => "Ctrl+Shift+\\",
            Command::ToggleSidebar => "Ctrl+\\",
            Command::Settings => "Ctrl+,",
            Command::PinNote => "Ctrl+Shift+P",
            _ => "",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Command::NewNote => Icon::Compose,
            Command::ToggleTheme => Icon::Palette,
            Command::FocusMode | Command::ToggleSidebar => Icon::Sidebar,
            Command::Settings => Icon::Settings,
            Command::PinNote => Icon::Pin,
            Command::DeleteNote => Icon::Trash,
            Command::RevealVault => Icon::External,
            Command::ChangeVault => Icon::FolderOpen,
        }
    }
}

#[derive(Default)]
struct Palette {
    open: bool,
    query: String,
    selected: usize,
    just_opened: bool,
}

pub struct YonoteApp {
    settings: Settings,
    saved_settings: Settings,
    vault: Option<Vault>,
    state: VaultState,
    filter: Filter,
    search: String,
    open: Option<OpenNote>,
    editor: Editor,
    error: Option<String>,
    toast: Option<Toast>,
    show_settings: bool,
    new_folder: Option<(PathBuf, String)>,
    palette: Palette,
    history: Vec<PathBuf>,
    history_pos: usize,
    was_focused: bool,
    close_anyway: bool,
    watcher: Option<notify::RecommendedWatcher>,
    fs_events: Option<mpsc::Receiver<()>>,
    rescan_at: Option<Instant>,
    applied_backdrop: Option<(Backdrop, bool)>,
    theme_mtime: Option<SystemTime>,
    theme_checked: Instant,
    fonts_for: String,
    show_backlinks: bool,
    /// Note selezionate nell'elenco (Ctrl/Maiusc+clic) e punto di partenza per Maiusc.
    selection: Vec<PathBuf>,
    anchor: Option<PathBuf>,
    confirm: Option<PendingDelete>,
    open_with_registered: bool,
    updater: crate::update::Updater,
    show_update: bool,
}

impl YonoteApp {
    pub fn new(cc: &eframe::CreationContext<'_>, file: Option<PathBuf>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        theme::ensure_example_themes();
        let settings = Settings::load();
        crate::i18n::set(settings.language);
        theme::install_fonts(&cc.egui_ctx, &settings.editor_font);
        theme::apply(&cc.egui_ctx, &settings);
        let mut app = Self {
            saved_settings: settings.clone(),
            fonts_for: settings.editor_font.clone(),
            theme_mtime: theme::theme_mtime(&settings.custom_theme),
            theme_checked: Instant::now(),
            settings,
            vault: None,
            state: VaultState::default(),
            filter: Filter::All,
            search: String::new(),
            open: None,
            editor: Editor::default(),
            error: None,
            toast: None,
            show_settings: false,
            new_folder: None,
            palette: Palette::default(),
            history: Vec::new(),
            history_pos: 0,
            was_focused: true,
            close_anyway: false,
            watcher: None,
            fs_events: None,
            rescan_at: None,
            applied_backdrop: None,
            show_backlinks: false,
            selection: Vec::new(),
            anchor: None,
            confirm: None,
            open_with_registered: crate::platform::is_registered(),
            updater: crate::update::Updater::default(),
            show_update: false,
        };
        app.apply_theme_mode(&cc.egui_ctx);
        if let Some(root) = app.settings.vault.clone() {
            app.open_vault(&cc.egui_ctx, root);
        }
        if app.settings.check_updates {
            app.updater.check(&cc.egui_ctx);
        }
        // Avviato con un file (doppio clic o "Apri con"): si apre e si modifica quel file.
        if let Some(f) = file {
            app.open_file(&f);
        }
        app
    }

    fn in_vault(&self, path: &Path) -> bool {
        self.vault.as_ref().is_some_and(|v| vault::is_inside(path, &v.root))
    }

    /// Apre un file Markdown qualsiasi. Se è nella cartella delle note lo mostra lì,
    /// altrimenti lo apre da solo e lo modifica direttamente (nessuna copia).
    fn open_file(&mut self, path: &Path) {
        let path = std::fs::canonicalize(path)
            .map(|p| PathBuf::from(p.display().to_string().trim_start_matches(r"\\?\")))
            .unwrap_or_else(|_| path.to_path_buf());
        if self.in_vault(&path) {
            self.filter = Filter::All;
        }
        self.open_note(&path);
    }

    fn apply_theme_mode(&self, ctx: &egui::Context) {
        ctx.set_theme(match self.settings.theme {
            ThemeMode::System => ThemePreference::System,
            ThemeMode::Light => ThemePreference::Light,
            ThemeMode::Dark => ThemePreference::Dark,
        });
    }

    fn open_vault(&mut self, ctx: &egui::Context, root: PathBuf) {
        self.flush(true);
        self.open = None;
        match Vault::open(root.clone()) {
            Ok(mut v) => {
                // La nota di benvenuto si crea solo la prima volta che si apre la cartella
                // (quando `.yonote/state.toml` non esiste ancora): se poi la si elimina, non torna.
                migrate_legacy_state_dir(&root);
                let first_time = !VaultState::path(&root).exists();
                if first_time && v.notes.is_empty() {
                    let welcome = root.join(format!("{}.md", tr("Benvenuto", "Welcome")));
                    let body = if crate::i18n::is_english() { WELCOME_EN } else { WELCOME_IT };
                    if fsutil::write_atomic(&welcome, body.as_bytes()).is_ok() {
                        v.upsert(&welcome, body);
                    }
                }
                self.vault = Some(v);
                self.state = VaultState::load(&root);
                if first_time {
                    self.state.save(&root);
                }
                self.settings.vault = Some(root.clone());
                self.filter = Filter::All;
                self.history.clear();
                self.start_watcher(ctx, &root);
                let last = self.state.last_open.clone().map(|r| root.join(r));
                let first = last.filter(|p| p.exists()).or_else(|| self.sorted_notes().first().map(|n| n.path.clone()));
                if let Some(p) = first {
                    self.open_note(&p);
                }
            }
            Err(e) => self.error = Some(format!("{}: {e}", tr("Impossibile aprire la cartella", "Could not open the folder"))),
        }
    }

    /// Osserva la cartella: le modifiche fatte da altri programmi compaiono subito.
    fn start_watcher(&mut self, ctx: &egui::Context, root: &Path) {
        use notify::Watcher;
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(ev) = res else { return };
            let relevant = ev.paths.iter().any(|p| {
                let s = p.to_string_lossy();
                !s.contains("\\.yonote") && !s.contains("\\.blocks") && !s.ends_with(".tmp")
            });
            if relevant {
                let _ = tx.send(());
                ctx.request_repaint_after(RESCAN_DELAY);
            }
        });
        match watcher {
            Ok(mut w) => {
                if w.watch(root, notify::RecursiveMode::Recursive).is_ok() {
                    self.watcher = Some(w);
                    self.fs_events = Some(rx);
                }
            }
            Err(e) => log_error(&format!("watcher: {e}")),
        }
    }

    fn poll_fs_events(&mut self) {
        if let Some(rx) = &self.fs_events
            && rx.try_iter().count() > 0
        {
            self.rescan_at = Some(Instant::now() + RESCAN_DELAY);
        }
        if self.rescan_at.is_some_and(|t| Instant::now() >= t) {
            self.rescan_at = None;
            self.reload_from_disk();
        }
    }

    fn choose_vault(&mut self, ctx: &egui::Context) {
        if let Some(dir) = rfd::FileDialog::new().set_title(tr("Scegli la cartella delle note", "Choose your notes folder")).pick_folder() {
            self.open_vault(ctx, dir);
        }
    }

    fn default_vault() -> PathBuf {
        directories::UserDirs::new()
            .and_then(|u| u.document_dir().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."))
            .join(tr("Note", "Notes"))
    }

    fn open_path(&self) -> Option<&Path> {
        self.open.as_ref().and_then(|o| o.path.as_deref())
    }

    fn open_note(&mut self, path: &Path) {
        self.open_note_inner(path, true);
    }

    fn open_note_inner(&mut self, path: &Path, record: bool) {
        if self.open_path().is_some_and(|p| vault::same_path(p, path)) {
            return;
        }
        self.flush(true);
        match vault::load_note(path) {
            Ok(n) => {
                self.open = Some(OpenNote {
                    path: Some(path.to_path_buf()),
                    dir: path.parent().map(Path::to_path_buf).unwrap_or_default(),
                    saved: n.text.clone(),
                    text: n.text,
                    crlf: n.crlf,
                    bom: n.bom,
                    mtime: n.mtime,
                    last_edit: Instant::now(),
                    edited: false,
                    last_failure: None,
                    external: !self.in_vault(path),
                });
                self.editor.reset();
                self.selection.clear();
                self.anchor = Some(path.to_path_buf());
                if record {
                    self.history.truncate(self.history_pos + usize::from(!self.history.is_empty()));
                    self.history.push(path.to_path_buf());
                    self.history_pos = self.history.len() - 1;
                }
                if let Some(v) = &self.vault
                    && vault::is_inside(path, &v.root)
                {
                    self.state.last_open = Some(rel(&v.root, path));
                    self.state.save(&v.root);
                }
            }
            Err(e) => self.error = Some(format!("{} «{}»: {e}", tr("Impossibile aprire", "Could not open"), path.display())),
        }
    }

    fn go_history(&mut self, delta: isize) {
        let target = self.history_pos as isize + delta;
        if target < 0 || target as usize >= self.history.len() {
            return;
        }
        let path = self.history[target as usize].clone();
        if path.exists() {
            self.history_pos = target as usize;
            self.open_note_inner(&path, false);
        }
    }

    fn new_note(&mut self) {
        let Some(root) = self.vault.as_ref().map(|v| v.root.clone()) else { return };
        self.flush(true);
        let dir = match &self.filter {
            Filter::Folder(f) => f.clone(),
            _ => root,
        };
        let text = match &self.filter {
            Filter::Tag(t) => format!("\n\n#{t}"),
            _ => String::new(),
        };
        self.open = Some(OpenNote {
            path: None,
            dir,
            saved: String::new(),
            text,
            crlf: false,
            bom: false,
            mtime: None,
            last_edit: Instant::now(),
            edited: false,
            last_failure: None,
            external: false,
        });
        self.editor.reset();
        self.editor.request_focus();
    }

    /// Apre la nota con quel titolo (o nome file); se non esiste la crea.
    fn open_or_create_by_title(&mut self, title: &str) {
        let Some(v) = &self.vault else { return };
        let wanted = title.to_lowercase();
        let found = v
            .notes
            .iter()
            .find(|n| n.title.to_lowercase() == wanted || n.path.file_stem().is_some_and(|s| s.to_string_lossy().to_lowercase() == wanted));
        if let Some(n) = found {
            let p = n.path.clone();
            self.open_note(&p);
        } else {
            self.new_note();
            if let Some(o) = &mut self.open {
                o.text = format!("{title}\n");
                o.edited = true;
            }
            self.flush(true);
            if let Some(p) = self.open_path().map(Path::to_path_buf) {
                self.history.push(p);
                self.history_pos = self.history.len() - 1;
            }
        }
    }

    /// Salva la nota aperta se serve. Con `force` ignora i tempi di attesa
    /// (cambio nota, chiusura).
    fn flush(&mut self, force: bool) {
        let Some(o) = &mut self.open else { return };
        let now = Instant::now();
        let waited = force || now.duration_since(o.last_edit) >= SAVE_DELAY;
        let may_retry = force || o.last_failure.is_none_or(|t| now.duration_since(t) >= RETRY_DELAY);

        if o.dirty() && waited && may_retry {
            let path = match &o.path {
                Some(p) => {
                    // Qualcun altro ha cambiato il file? Allora non lo sovrascriviamo:
                    // la nostra versione va in una copia di conflitto.
                    let disk = std::fs::metadata(p).and_then(|m| m.modified()).ok();
                    if disk.is_some() && o.mtime.is_some() && disk != o.mtime {
                        let c = vault::conflict_path(p);
                        self.error = Some(format!(
                            "{} «{}».",
                            tr(
                                "La nota è stata modificata da un altro programma. La tua versione è stata salvata come",
                                "The note was changed by another program. Your version was saved as"
                            ),
                            c.file_name().unwrap_or_default().to_string_lossy()
                        ));
                        c
                    } else {
                        p.clone()
                    }
                }
                None if o.text.trim().is_empty() => return,
                None => vault::unique_path(&o.dir, &vault::file_stem_for_title(&vault::title_of(&o.text)), None),
            };
            match fsutil::write_atomic(&path, &vault::encode(&o.text, o.crlf, o.bom)) {
                Ok(()) => {
                    o.saved = o.text.clone();
                    o.mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
                    o.last_failure = None;
                    if let Some(v) = &mut self.vault
                        && !o.external
                    {
                        v.upsert(&path, &o.text);
                    }
                    if o.path.is_none()
                        && let Some(v) = &self.vault
                    {
                        self.state.last_open = Some(rel(&v.root, &path));
                        self.state.save(&v.root);
                        if self.history.last().is_none_or(|h| !vault::same_path(h, &path)) {
                            self.history.push(path.clone());
                            self.history_pos = self.history.len() - 1;
                        }
                    }
                    o.path = Some(path);
                    if self.error.as_deref().is_some_and(|e| e.starts_with(tr("Impossibile salvare", "Could not save"))) {
                        self.error = None;
                    }
                }
                Err(e) => {
                    o.last_failure = Some(now);
                    self.error = Some(format!(
                        "{}: {e}. {}",
                        tr("Impossibile salvare", "Could not save"),
                        tr("Riprovo tra poco…", "Retrying shortly…")
                    ));
                }
            }
        }

        // Rinomina il file quando il titolo è stabile.
        let rename_ready = force || now.duration_since(o.last_edit) >= RENAME_DELAY;
        let Some(path) = o.path.clone().filter(|_| o.edited && !o.dirty() && rename_ready) else { return };
        // Le note con frontmatter (`---` iniziale) e i file aperti da fuori non vengono rinominati.
        if o.text.starts_with("---") || o.external {
            return;
        }
        let Some(v) = &mut self.vault else { return };
        let wanted = vault::file_stem_for_title(&vault::title_of(&o.text));
        let current = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        if wanted == current || current.starts_with(&format!("{wanted} ")) {
            return;
        }
        let dir = path.parent().unwrap_or(&v.root).to_path_buf();
        let target = vault::unique_path(&dir, &wanted, Some(&path));
        if target == path {
            return;
        }
        match std::fs::rename(&path, &target) {
            Ok(()) => {
                v.remove(&path);
                v.upsert(&target, &o.text);
                o.mtime = std::fs::metadata(&target).and_then(|m| m.modified()).ok();
                let (old_rel, new_rel) = (rel(&v.root, &path), rel(&v.root, &target));
                for p in &mut self.state.pinned {
                    if *p == old_rel {
                        p.clone_from(&new_rel);
                    }
                }
                for h in &mut self.history {
                    if vault::same_path(h, &path) {
                        h.clone_from(&target);
                    }
                }
                self.state.last_open = Some(new_rel);
                self.state.save(&v.root);
                let new_stem = target.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let failures = update_links(v, &target, &current, &new_stem);
                if failures > 0 {
                    self.error =
                        Some(tr("Alcuni link alla nota non sono stati aggiornati.", "Some links to the note could not be updated.").into());
                }
                o.path = Some(target);
            }
            Err(e) => self.error = Some(format!("{}: {e}", tr("Impossibile rinominare la nota", "Could not rename the note"))),
        }
    }

    /// Chiede conferma prima di spostare le note nel Cestino.
    fn request_delete(&mut self, paths: Vec<PathBuf>) {
        if !paths.is_empty() {
            self.confirm = Some(PendingDelete::Notes(paths));
        }
    }

    /// Note su cui agisce "elimina": quelle selezionate o, se non ce ne sono, quella aperta.
    fn delete_targets(&self, clicked: Option<&Path>) -> Vec<PathBuf> {
        match clicked {
            Some(c) if self.selection.iter().any(|s| vault::same_path(s, c)) && self.selection.len() > 1 => self.selection.clone(),
            Some(c) => vec![c.to_path_buf()],
            None if !self.selection.is_empty() => self.selection.clone(),
            None => self.open_path().map(|p| vec![p.to_path_buf()]).unwrap_or_default(),
        }
    }

    fn note_title(&self, path: &Path) -> String {
        self.vault
            .as_ref()
            .and_then(|v| v.notes.iter().find(|n| vault::same_path(&n.path, path)))
            .map(|n| n.title.clone())
            .unwrap_or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
    }

    fn delete_notes(&mut self, paths: &[PathBuf]) {
        if self.open_path().is_some_and(|p| paths.iter().any(|d| vault::same_path(p, d))) {
            self.flush(true);
            self.open = None;
        }
        let first_title = paths.first().map(|p| self.note_title(p)).unwrap_or_default();
        let mut deleted = Vec::new();
        for path in paths {
            match trash::delete(path) {
                Ok(()) => {
                    if let Some(v) = &mut self.vault {
                        v.remove(path);
                        let r = rel(&v.root, path);
                        self.state.pinned.retain(|p| *p != r);
                    }
                    deleted.push(path.clone());
                }
                Err(e) => {
                    self.error = Some(format!(
                        "{}: {e}",
                        tr("Impossibile spostare la nota nel Cestino", "Could not move the note to the Recycle Bin")
                    ));
                }
            }
        }
        if let Some(v) = &self.vault {
            self.state.save(&v.root);
        }
        self.selection.clear();
        if !deleted.is_empty() {
            let text = if deleted.len() == 1 {
                format!("«{first_title}» {}", tr("spostata nel Cestino", "moved to the Recycle Bin"))
            } else {
                format!("{} {}", deleted.len(), tr("note spostate nel Cestino", "notes moved to the Recycle Bin"))
            };
            self.toast = Some(Toast::new(text, deleted, TOAST_TIME));
        }
        if self.open.is_none()
            && let Some(p) = self.sorted_notes().first().map(|n| n.path.clone())
        {
            self.open_note(&p);
        }
    }

    /// Ripristina dal Cestino di Windows le note appena eliminate.
    fn undo_delete(&mut self, paths: &[PathBuf]) {
        let mut restored = None;
        for path in paths {
            match vault::restore_from_trash(path) {
                Ok(()) => restored = restored.or(Some(path.clone())),
                Err(e) => self.error = Some(format!("{}: {e}", tr("Impossibile ripristinare la nota", "Could not restore the note"))),
            }
        }
        self.reload_from_disk();
        if let Some(p) = restored {
            self.open_note(&p);
        }
        self.toast = None;
    }

    fn delete_folder(&mut self, folder: &Path) {
        if self.open_path().is_some_and(|p| vault::is_inside(p, folder)) {
            self.flush(true);
            self.open = None;
        }
        match trash::delete(folder) {
            Ok(()) => {
                self.filter = Filter::All;
                self.reload_from_disk();
                let name = folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                self.toast = Some(Toast::new(
                    format!("«{name}» {}", tr("spostata nel Cestino", "moved to the Recycle Bin")),
                    Vec::new(),
                    TOAST_TIME,
                ));
            }
            Err(e) => self.error = Some(format!("{}: {e}", tr("Impossibile eliminare la cartella", "Could not delete the folder"))),
        }
    }

    fn move_note(&mut self, path: &Path, folder: &Path) {
        let is_open = self.open_path().is_some_and(|p| vault::same_path(p, path));
        if is_open {
            self.flush(true);
        }
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let target = vault::unique_path(folder, &stem, None);
        match std::fs::rename(path, &target) {
            Ok(()) => {
                if let Some(v) = &mut self.vault {
                    let (old_rel, new_rel) = (rel(&v.root, path), rel(&v.root, &target));
                    for p in &mut self.state.pinned {
                        if *p == old_rel {
                            p.clone_from(&new_rel);
                        }
                    }
                    self.state.save(&v.root);
                    v.rescan();
                }
                if is_open && let Some(o) = &mut self.open {
                    o.path = Some(target.clone());
                    o.dir = folder.to_path_buf();
                    o.mtime = std::fs::metadata(&target).and_then(|m| m.modified()).ok();
                }
            }
            Err(e) => self.error = Some(format!("{}: {e}", tr("Impossibile spostare la nota", "Could not move the note"))),
        }
    }

    fn toggle_pin(&mut self, path: &Path) {
        let Some(v) = &self.vault else { return };
        let r = rel(&v.root, path);
        if let Some(i) = self.state.pinned.iter().position(|p| *p == r) {
            self.state.pinned.remove(i);
        } else {
            self.state.pinned.push(r);
        }
        self.state.save(&v.root);
    }

    fn is_pinned(&self, path: &Path) -> bool {
        self.vault.as_ref().is_some_and(|v| self.state.pinned.contains(&rel(&v.root, path)))
    }

    /// Rilegge il vault dal disco e, se la nota aperta non ha modifiche
    /// in sospeso ed è cambiata su disco, la ricarica.
    fn reload_from_disk(&mut self) {
        let Some(v) = &mut self.vault else { return };
        v.rescan();
        if let Some(o) = &mut self.open
            && let Some(p) = &o.path
        {
            let disk = std::fs::metadata(p).and_then(|m| m.modified()).ok();
            if disk.is_none() {
                // Eliminata o spostata da fuori: resta aperta come nota nuova, nulla va perso.
                o.path = None;
                o.saved.clear();
            } else if !o.dirty()
                && disk != o.mtime
                && let Ok(n) = vault::load_note(p)
            {
                o.saved = n.text.clone();
                o.text = n.text;
                o.mtime = n.mtime;
                self.editor.reset();
            }
        }
    }

    fn sorted_notes(&self) -> Vec<&NoteMeta> {
        let Some(v) = &self.vault else { return Vec::new() };
        let terms: Vec<String> = vault::fold(&self.search).split_whitespace().map(str::to_owned).collect();
        let mut notes: Vec<&NoteMeta> = v
            .notes
            .iter()
            .filter(|n| match &self.filter {
                Filter::All => true,
                Filter::Folder(f) => n.path.parent().is_some_and(|p| vault::same_path(p, f)),
                Filter::Tag(t) => n.tags.iter().any(|x| x == t || x.starts_with(&format!("{t}/"))),
            })
            .filter(|n| {
                terms.iter().all(|t| match t.strip_prefix('#') {
                    Some(tag) if !tag.is_empty() => n.tags.iter().any(|x| x.starts_with(tag)),
                    _ => n.search.contains(t.as_str()),
                })
            })
            .collect();
        notes.sort_by_key(|n| std::cmp::Reverse(n.modified));
        notes
    }

    fn filter_title(&self) -> String {
        match &self.filter {
            Filter::All => tr("Tutte le note", "All notes").to_owned(),
            Filter::Folder(f) => f.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
            Filter::Tag(t) => format!("#{t}"),
        }
    }

    fn run_command(&mut self, ctx: &egui::Context, cmd: Command) {
        match cmd {
            Command::NewNote => self.new_note(),
            Command::ToggleTheme => {
                let dark = ctx.theme() == egui::Theme::Dark;
                self.settings.theme = if dark { ThemeMode::Light } else { ThemeMode::Dark };
            }
            Command::FocusMode => {
                let any = self.settings.sidebar_open || self.settings.list_open;
                self.settings.sidebar_open = !any;
                self.settings.list_open = !any;
            }
            Command::ToggleSidebar => self.settings.sidebar_open = !self.settings.sidebar_open,
            Command::Settings => self.show_settings = true,
            Command::PinNote => {
                if let Some(p) = self.open_path().map(Path::to_path_buf) {
                    self.toggle_pin(&p);
                }
            }
            Command::DeleteNote => {
                let targets = self.delete_targets(None);
                self.request_delete(targets);
            }
            Command::RevealVault => {
                if let Some(v) = &self.vault {
                    crate::platform::open_folder(&v.root);
                }
            }
            Command::ChangeVault => self.choose_vault(ctx),
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if std::env::var_os("BLOCKS_DEBUG_KEYS").is_some() {
            ctx.input(|i| {
                for e in &i.events {
                    if let egui::Event::Key { key, physical_key, pressed: true, modifiers, .. } = e {
                        log_error(&format!("key {key:?} phys {physical_key:?} mods {modifiers:?}"));
                    }
                }
            });
        }
        let cmd = Modifiers::COMMAND;
        let pressed = |m: Modifiers, k: Key| shortcut(ctx, m, k);
        if pressed(cmd, Key::N) {
            self.new_note();
        }
        if pressed(cmd | Modifiers::SHIFT, Key::Backslash) {
            self.run_command(ctx, Command::FocusMode);
        }
        if pressed(cmd, Key::Backslash) {
            self.settings.sidebar_open = !self.settings.sidebar_open;
        }
        if pressed(cmd, Key::Comma) {
            self.show_settings = !self.show_settings;
        }
        if (!self.editor.has_selection() && pressed(cmd, Key::K)) || pressed(cmd, Key::P) {
            self.palette = Palette { open: true, just_opened: true, ..Default::default() };
        }
        if pressed(cmd | Modifiers::SHIFT, Key::F) {
            self.settings.list_open = true;
            ctx.memory_mut(|m| m.request_focus(Id::unique("yonote-search")));
        }
        if pressed(cmd | Modifiers::SHIFT, Key::P) {
            self.run_command(ctx, Command::PinNote);
        }
        // Canc: elimina le note selezionate (solo se non si sta scrivendo in un campo di testo).
        if !ctx.egui_wants_keyboard_input() && pressed(Modifiers::NONE, Key::Delete) {
            let targets = self.delete_targets(None);
            self.request_delete(targets);
        }
        if !ctx.egui_wants_keyboard_input() && !self.selection.is_empty() && pressed(Modifiers::NONE, Key::Escape) {
            self.selection.clear();
        }
        if pressed(Modifiers::ALT, Key::ArrowLeft) {
            self.go_history(-1);
        }
        if pressed(Modifiers::ALT, Key::ArrowRight) {
            self.go_history(1);
        }
        if pressed(cmd, Key::Equals) || pressed(cmd, Key::Plus) {
            self.settings.font_size = (self.settings.font_size + 1.0).min(32.0);
        }
        if pressed(cmd, Key::Minus) {
            self.settings.font_size = (self.settings.font_size - 1.0).max(10.0);
        }
    }

    /// Applica Mica/Acrylic alla finestra quando cambia lo sfondo o il tema.
    fn sync_backdrop(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let dark = ctx.theme() == egui::Theme::Dark;
        let want = (self.settings.backdrop, dark);
        if self.applied_backdrop == Some(want) {
            return;
        }
        let _ = window_vibrancy::clear_mica(frame);
        let _ = window_vibrancy::clear_acrylic(frame);
        let result = match self.settings.backdrop {
            Backdrop::Off => Ok(()),
            Backdrop::Mica => window_vibrancy::apply_mica(frame, Some(dark)),
            Backdrop::Acrylic => {
                let tint = if dark { (24, 24, 24, 90) } else { (245, 245, 245, 90) };
                window_vibrancy::apply_acrylic(frame, Some(tint))
            }
        };
        if let Err(e) = result {
            log_error(&format!("backdrop: {e}"));
            if self.settings.backdrop != Backdrop::Off {
                self.error = Some(
                    tr(
                        "Questo Windows non supporta lo sfondo trasparente: uso lo sfondo pieno.",
                        "This Windows version doesn't support the transparent background: using a solid one.",
                    )
                    .into(),
                );
                self.settings.backdrop = Backdrop::Off;
            }
        }
        self.applied_backdrop = Some((self.settings.backdrop, dark));
        // Windows spegne Mica/Acrylic quando la finestra perde il focus: lo teniamo acceso.
        if let Some(h) = crate::platform::hwnd(frame) {
            crate::platform::keep_backdrop_when_inactive(h, self.settings.backdrop != Backdrop::Off);
        }
    }

    /// Applica i cambiamenti delle impostazioni e le salva.
    fn sync_settings(&mut self, ctx: &egui::Context) {
        // Tema da file modificato su disco: ricaricalo (controllo al massimo una volta al secondo).
        let mut theme_file_changed = false;
        let mut mtime = self.theme_mtime;
        if !self.settings.custom_theme.is_empty() && self.theme_checked.elapsed() >= Duration::from_secs(1) {
            self.theme_checked = Instant::now();
            mtime = theme::theme_mtime(&self.settings.custom_theme);
            theme_file_changed = mtime != self.theme_mtime;
        }
        if self.settings == self.saved_settings && !theme_file_changed {
            return;
        }
        let old = self.saved_settings.clone();
        if self.settings.theme != old.theme {
            self.apply_theme_mode(ctx);
        }
        if self.settings.language != old.language {
            crate::i18n::set(self.settings.language);
        }
        if self.settings.editor_font != self.fonts_for {
            theme::install_fonts(ctx, &self.settings.editor_font);
            self.fonts_for = self.settings.editor_font.clone();
        }
        if self.settings.accent != old.accent || self.settings.custom_theme != old.custom_theme || theme_file_changed {
            theme::apply(ctx, &self.settings);
            self.theme_mtime = mtime;
            self.applied_backdrop = None;
        }
        // Salva al massimo una volta mentre si trascina un cursore: basta a fine trascinamento.
        if ctx.input(|i| i.pointer.any_down()) {
            return;
        }
        match self.settings.save() {
            Ok(()) => self.saved_settings = self.settings.clone(),
            Err(e) => log_error(&format!("settings: {e}")),
        }
    }
}

impl eframe::App for YonoteApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Con Mica/Acrylic la finestra è trasparente e l'effetto di Windows si vede dietro i pannelli.
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.sync_backdrop(&ctx, frame);
        self.poll_fs_events();

        // Ricarica quando la finestra torna in primo piano, salva quando la perde.
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if focused && !self.was_focused {
            self.reload_from_disk();
        }
        if !focused && self.was_focused {
            self.flush(true);
        }
        self.was_focused = focused;

        // Chiusura: salva tutto; se non ci si riesce, chiede una seconda conferma.
        if ctx.input(|i| i.viewport().close_requested()) {
            self.flush(true);
            if self.open.as_ref().is_some_and(OpenNote::dirty) && !self.close_anyway {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.error = Some(
                    tr(
                        "Impossibile salvare la nota. Chiudi di nuovo per uscire comunque (le modifiche andranno perse).",
                        "Could not save the note. Close again to quit anyway (changes will be lost).",
                    )
                    .into(),
                );
                self.close_anyway = true;
            }
        }

        if !self.palette.open {
            self.shortcuts(&ctx);
        }
        // Base opaca quando lo sfondo trasparente è spento.
        if self.settings.backdrop == Backdrop::Off {
            ui.painter().rect_filled(ctx.content_rect(), 0.0, theme::palette(&ctx).ground);
        }
        self.error_banner(ui);

        if self.vault.is_none() && self.open.is_none() {
            self.welcome(ui);
        } else if self.vault.is_none() {
            // Solo un file aperto con "Apri con", senza cartella delle note.
            let [_, _, editor_fill] = theme::panel_fills(&ctx, &self.settings);
            egui::CentralPanel::default().frame(egui::Frame::new().fill(editor_fill)).show(ui, |ui| self.editor_area(ui));
        } else {
            let width = ui.available_width();
            let mut sidebar_open = self.settings.sidebar_open && width >= 900.0;
            let mut list_open = self.settings.list_open && width >= 600.0;
            let [side_fill, list_fill, editor_fill] = theme::panel_fills(&ctx, &self.settings);
            egui::Panel::left("sidebar")
                .resizable(true)
                .default_size(232.0)
                .size_range(180.0..=360.0)
                .show_separator_line(true)
                .frame(egui::Frame::new().fill(side_fill).inner_margin(egui::Margin { left: 10, right: 10, top: 4, bottom: 4 }))
                .show_collapsible(ui, &mut sidebar_open, |ui| self.sidebar(ui));
            egui::Panel::left("list")
                .resizable(true)
                .default_size(320.0)
                .size_range(240.0..=520.0)
                .show_separator_line(true)
                .frame(egui::Frame::new().fill(list_fill).inner_margin(egui::Margin { left: 10, right: 10, top: 4, bottom: 0 }))
                .show_collapsible(ui, &mut list_open, |ui| self.note_list(ui));
            egui::CentralPanel::default().frame(egui::Frame::new().fill(editor_fill)).show(ui, |ui| self.editor_area(ui));

            // Angolo in basso a sinistra: impostazioni e colonne.
            if sidebar_open {
                egui::Area::new(Id::unique("corner")).anchor(Align2::LEFT_BOTTOM, Vec2::new(12.0, -10.0)).show(&ctx, |ui| {
                    ui.horizontal(|ui| {
                        if icons::button(ui, Icon::Settings, tr("Impostazioni (Ctrl+,)", "Settings (Ctrl+,)")).clicked() {
                            self.show_settings = !self.show_settings;
                        }
                        if icons::button(ui, Icon::Sidebar, tr("Nascondi le cartelle (Ctrl+\\)", "Hide folders (Ctrl+\\)")).clicked() {
                            self.settings.sidebar_open = false;
                        }
                        // Aggiornamento disponibile: una pillola discreta, mai una finestra che si apre da sola.
                        let version = match self.updater.status() {
                            crate::update::Status::Available(r)
                            | crate::update::Status::Downloading(r, _)
                            | crate::update::Status::Ready(r, _) => Some(r.version),
                            _ => None,
                        };
                        if let Some(v) = version {
                            ui.add_space(2.0);
                            if widgets::pill(ui, Icon::Update, &format!("{} {v}", tr("Aggiorna a", "Update to")))
                                .on_hover_text(tr("È disponibile una nuova versione di Yonote", "A new version of Yonote is available"))
                                .clicked()
                            {
                                self.show_update = true;
                            }
                        }
                    });
                });
            }
        }

        self.palette_window(&ctx);
        self.settings_window(&ctx);
        self.new_folder_window(&ctx);
        self.confirm_window(&ctx);
        self.update_window(&ctx);
        self.toast_ui(&ctx);

        // Salvataggio automatico.
        self.flush(false);
        if let Some(o) = &self.open
            && (o.dirty() || o.edited)
        {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        self.sync_settings(&ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.flush(true);
        let _ = self.settings.save();
    }
}

/// Scorciatoia da tastiera: accetta sia il tasto logico sia quello fisico,
/// così funziona con qualsiasi layout (es. la virgola sulla tastiera italiana).
fn shortcut(ctx: &egui::Context, mods: Modifiers, key: Key) -> bool {
    if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(mods, key))) {
        return true;
    }
    ctx.input_mut(|i| {
        let pos = i.events.iter().position(|e| {
            matches!(e, egui::Event::Key { physical_key: Some(pk), pressed: true, modifiers, .. }
                if *pk == key && modifiers.matches_logically(mods))
        });
        if let Some(p) = pos {
            i.events.remove(p);
        }
        pos.is_some()
    })
}

/// Aggiorna i `[[link]]` nelle altre note dopo una rinomina. Restituisce quante note non è riuscito a scrivere.
fn update_links(v: &mut Vault, renamed: &Path, old: &str, new: &str) -> usize {
    let old_l = old.to_lowercase();
    let targets: Vec<PathBuf> =
        v.notes.iter().filter(|n| !vault::same_path(&n.path, renamed) && n.links.contains(&old_l)).map(|n| n.path.clone()).collect();
    let mut failures = 0;
    for path in targets {
        let Ok(n) = vault::load_note(&path) else {
            failures += 1;
            continue;
        };
        if let Some(text) = crate::markdown::rename_links(&n.text, old, new) {
            if fsutil::write_atomic(&path, &vault::encode(&text, n.crlf, n.bom)).is_ok() {
                v.upsert(&path, &text);
            } else {
                failures += 1;
            }
        }
    }
    failures
}

pub fn log_error(msg: &str) {
    if let Some(dir) = Settings::dir() {
        use std::io::Write;
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("yonote.log")) {
            let _ = writeln!(f, "{} {msg}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
        }
    }
}

const WELCOME_IT: &str = include_str!("../../assets/welcome_it.md");

const WELCOME_EN: &str = include_str!("../../assets/welcome_en.md");
