//! Il "vault": la cartella dell'utente con le note `.md`.
//!
//! Le note sono file Markdown normali. L'app non aggiunge metadati ai file.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Cartella delle immagini incollate (nascosta dall'albero).
pub const ATTACHMENTS_DIR: &str = "_allegati";
pub fn untitled() -> &'static str {
    crate::i18n::tr("Senza titolo", "Untitled")
}

/// Testo per la ricerca: minuscolo e senza accenti (`Città` → `citta`).
pub fn fold(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfd().filter(|c| !unicode_normalization::char::is_combining_mark(*c)).flat_map(char::to_lowercase).collect()
}

/// Dati di una nota necessari all'elenco e alla ricerca.
#[derive(Clone, Debug)]
pub struct NoteMeta {
    pub path: PathBuf,
    pub title: String,
    pub preview: String,
    pub modified: SystemTime,
    /// Testo completo minuscolo e senza accenti, per la ricerca.
    pub search: String,
    /// `#tag` della nota, in minuscolo.
    pub tags: Vec<String>,
    /// Note collegate con `[[ ]]`, in minuscolo.
    pub links: Vec<String>,
    /// Prima immagine della nota (percorso assoluto), per la miniatura.
    pub image: Option<PathBuf>,
}

impl NoteMeta {
    pub fn from_text(path: PathBuf, text: &str, modified: SystemTime) -> Self {
        let title = title_of(text);
        let title = if title.is_empty() { path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default() } else { title };
        let lines = crate::markdown::parse(text);
        let image = lines.iter().find_map(|l| l.src.as_deref()).and_then(|src| {
            let dir = path.parent()?;
            let p = resolve_link(dir, src);
            p.exists().then_some(p)
        });
        Self {
            preview: preview_of(text),
            search: fold(text),
            tags: crate::markdown::tags(text),
            links: crate::markdown::links(text),
            image,
            title,
            path,
            modified,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Folder {
    pub path: PathBuf,
    pub name: String,
    pub children: Vec<Folder>,
}

pub struct Vault {
    pub root: PathBuf,
    pub notes: Vec<NoteMeta>,
    pub tree: Folder,
    /// Titoli di tutte le note (suggerimenti `[[`).
    pub titles: Vec<String>,
    /// Tag con il numero di note, in ordine alfabetico.
    pub tags: Vec<(String, usize)>,
    /// Solo i nomi dei tag (suggerimenti `#`).
    pub tag_names: Vec<String>,
}

impl Vault {
    pub fn open(root: PathBuf) -> anyhow::Result<Self> {
        fs::create_dir_all(&root)?;
        let mut vault = Self {
            tree: Folder { path: root.clone(), name: String::new(), children: Vec::new() },
            notes: Vec::new(),
            titles: Vec::new(),
            tags: Vec::new(),
            tag_names: Vec::new(),
            root,
        };
        vault.rescan();
        Ok(vault)
    }

    /// Rilegge cartelle e note dal disco.
    pub fn rescan(&mut self) {
        let mut notes = Vec::new();
        self.tree = scan_dir(&self.root, &mut notes);
        self.tree.name.clear();
        self.notes = notes;
        self.reindex();
    }

    /// Ricalcola titoli e tag: si fa quando le note cambiano, non a ogni fotogramma.
    fn reindex(&mut self) {
        self.titles = self.notes.iter().map(|n| n.title.clone()).collect();
        let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for t in self.notes.iter().flat_map(|n| &n.tags) {
            *counts.entry(t).or_default() += 1;
        }
        self.tags = counts.into_iter().map(|(t, c)| (t.to_owned(), c)).collect();
        self.tag_names = self.tags.iter().map(|(t, _)| t.clone()).collect();
    }

    /// Aggiorna (o aggiunge) i dati di una nota dopo un salvataggio.
    pub fn upsert(&mut self, path: &Path, text: &str) {
        let modified = fs::metadata(path).and_then(|m| m.modified()).unwrap_or_else(|_| SystemTime::now());
        let meta = NoteMeta::from_text(path.to_path_buf(), text, modified);
        match self.notes.iter_mut().find(|n| same_path(&n.path, path)) {
            Some(n) => *n = meta,
            None => self.notes.push(meta),
        }
        self.reindex();
    }

    pub fn remove(&mut self, path: &Path) {
        self.notes.retain(|n| !same_path(&n.path, path));
        self.reindex();
    }
}

fn is_hidden_entry(name: &str) -> bool {
    name.starts_with('.') || name.eq_ignore_ascii_case(ATTACHMENTS_DIR)
}

fn scan_dir(dir: &Path, notes: &mut Vec<NoteMeta>) -> Folder {
    let mut folder = Folder {
        path: dir.to_path_buf(),
        name: dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        children: Vec::new(),
    };
    let Ok(entries) = fs::read_dir(dir) else { return folder };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_hidden_entry(&name) {
            continue;
        }
        let Ok(ft) = entry.file_type() else { continue };
        let path = entry.path();
        if ft.is_dir() {
            folder.children.push(scan_dir(&path, notes));
        } else if ft.is_file() && is_markdown(&path) {
            let modified = entry.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
            let text = fs::read(&path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
            notes.push(NoteMeta::from_text(path, &text, modified));
        }
    }
    folder.children.sort_by_key(|f| f.name.to_lowercase());
    folder
}

pub fn is_markdown(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

/// `path` si trova dentro `dir` (a qualsiasi profondità)?
pub fn is_inside(path: &Path, dir: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    let (p, d) = (norm(path), norm(dir));
    p.len() > d.len() && p.starts_with(&d) && p[d.len()..].starts_with('\\')
}

/// Confronto di percorsi come fa Windows (senza distinguere maiuscole/minuscole).
pub fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    norm(a) == norm(b)
}

// ---------------------------------------------------------------------------
// Lettura e scrittura del contenuto

/// Una nota aperta nell'editor, con le informazioni per riscriverla identica.
#[derive(Debug)]
pub struct LoadedNote {
    /// Testo con terminazioni di riga `\n`.
    pub text: String,
    pub crlf: bool,
    pub bom: bool,
    pub mtime: Option<SystemTime>,
}

/// Legge una nota. Fallisce se il file non è UTF-8 valido, così non verrà mai
/// riscritto rovinandolo.
pub fn load_note(path: &Path) -> anyhow::Result<LoadedNote> {
    let bytes = fs::read(path)?;
    let mtime = fs::metadata(path).and_then(|m| m.modified()).ok();
    let (bom, body) = match bytes.strip_prefix(b"\xEF\xBB\xBF") {
        Some(rest) => (true, rest),
        None => (false, &bytes[..]),
    };
    let raw = std::str::from_utf8(body).map_err(|_| anyhow::anyhow!("il file non è testo UTF-8"))?;
    let crlf = raw.contains("\r\n");
    let text = if crlf { raw.replace("\r\n", "\n") } else { raw.to_owned() };
    Ok(LoadedNote { text, crlf, bom, mtime })
}

/// Converte il testo dell'editor nei byte da scrivere, rispettando il formato originale.
pub fn encode(text: &str, crlf: bool, bom: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 3);
    if bom {
        out.extend_from_slice(b"\xEF\xBB\xBF");
    }
    if crlf {
        out.extend_from_slice(text.replace('\n', "\r\n").as_bytes());
    } else {
        out.extend_from_slice(text.as_bytes());
    }
    out
}

// ---------------------------------------------------------------------------
// Titolo e anteprima

/// Il titolo è la prima riga non vuota, senza i `#` iniziali.
pub fn title_of(text: &str) -> String {
    text.lines().map(str::trim).find(|l| !l.is_empty()).map(|l| l.trim_start_matches('#').trim().to_owned()).unwrap_or_default()
}

/// Seconda riga non vuota, ripulita dalla sintassi Markdown più comune.
pub fn preview_of(text: &str) -> String {
    let line = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("![")).nth(1).unwrap_or("");
    let mut s = line.trim_start_matches(['#', '>', ' ']).to_owned();
    for prefix in ["- [ ] ", "- [x] ", "- [X] ", "- ", "* "] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.to_owned();
            break;
        }
    }
    let s: String = s.chars().filter(|c| !matches!(c, '*' | '`' | '~' | '[' | ']')).collect();
    truncate_chars(s.trim(), 90)
}

fn truncate_chars(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", s[..i].trim_end()),
        None => s.to_owned(),
    }
}

/// Trasforma un titolo in un nome di file valido per Windows (senza `.md`).
pub fn file_stem_for_title(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { '-' } else { c })
        .collect();
    let mut stem = truncate_chars(cleaned.trim(), 100).trim_end_matches('…').to_owned();
    while stem.ends_with(['.', ' ']) {
        stem.pop();
    }
    let stem = stem.trim_start().to_owned();
    if stem.is_empty() {
        return untitled().to_owned();
    }
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4",
        "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let base = stem.split('.').next().unwrap_or("").trim();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(base)) { format!("{stem}_") } else { stem }
}

/// Primo percorso libero `dir/stem.md`, `dir/stem 2.md`, …
/// `current` è il file della nota stessa: tenerlo non è un conflitto.
pub fn unique_path(dir: &Path, stem: &str, current: Option<&Path>) -> PathBuf {
    for n in 1.. {
        let name = if n == 1 { format!("{stem}.md") } else { format!("{stem} {n}.md") };
        let candidate = dir.join(name);
        if current.is_some_and(|c| same_path(c, &candidate)) || !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

/// Percorso di un'immagine scritta in una nota (relativo alla cartella della nota,
/// con `/` o `\`, eventuali `%20`).
pub fn resolve_link(note_dir: &Path, src: &str) -> PathBuf {
    let src = src.replace("%20", " ");
    let p = Path::new(&src);
    if p.is_absolute() { p.to_path_buf() } else { note_dir.join(src.replace('/', "\\")) }
}

/// Percorso relativo da `from_dir` a `to`, con `/` (come si scrive nel Markdown).
pub fn relative_link(from_dir: &Path, to: &Path) -> String {
    let from: Vec<_> = from_dir.components().collect();
    let to_c: Vec<_> = to.components().collect();
    let common = from
        .iter()
        .zip(&to_c)
        .take_while(|(a, b)| a.as_os_str().to_string_lossy().to_lowercase() == b.as_os_str().to_string_lossy().to_lowercase())
        .count();
    let mut parts: Vec<String> = std::iter::repeat_n("..".to_owned(), from.len() - common).collect();
    parts.extend(to_c[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    parts.join("/").replace(' ', "%20")
}

/// Salva un'immagine incollata in `_allegati` e restituisce il suo percorso.
pub fn save_attachment(root: &Path, png: &[u8], ext: &str) -> std::io::Result<PathBuf> {
    let dir = root.join(ATTACHMENTS_DIR);
    let stamp = chrono::Local::now().format("%Y-%m-%d-%H%M%S");
    let mut path = dir.join(format!("{stamp}.{ext}"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{stamp}-{n}.{ext}"));
        n += 1;
    }
    crate::fsutil::write_atomic(&path, png)?;
    Ok(path)
}

/// Ripristina dal Cestino di Windows l'ultimo file eliminato con questo percorso.
pub fn restore_from_trash(path: &Path) -> Result<(), String> {
    let items = trash::os_limited::list().map_err(|e| e.to_string())?;
    let item = items
        .into_iter()
        .filter(|i| same_path(&i.original_parent.join(&i.name), path))
        .max_by_key(|i| i.time_deleted)
        .ok_or_else(|| crate::i18n::tr("non trovata nel Cestino", "not found in the Recycle Bin").to_owned())?;
    trash::os_limited::restore_all([item]).map_err(|e| e.to_string())
}

/// Percorso per salvare una copia in conflitto senza perdere nulla.
pub fn conflict_path(path: &Path) -> PathBuf {
    let dir = path.parent().unwrap_or(Path::new("."));
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let stamp = chrono::Local::now().format("%Y-%m-%d %H.%M");
    unique_path(dir, &format!("{stem} (conflitto {stamp})"), None)
}

/// Ricerca "fuzzy": le voci più simili a `query`, al massimo `limit`.
pub fn fuzzy_rank(query: &str, items: &[String], limit: usize) -> Vec<String> {
    use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
    if query.is_empty() {
        return items.iter().take(limit).cloned().collect();
    }
    let mut m = nucleo_matcher::Matcher::new(nucleo_matcher::Config::DEFAULT);
    let mut hits = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart).match_list(items.iter(), &mut m);
    hits.sort_by_key(|h| std::cmp::Reverse(h.1));
    hits.into_iter().take(limit).map(|(s, _)| s.clone()).collect()
}

/// URI `file:///C:/…` per caricare un'immagine con egui.
pub fn file_uri(path: &Path) -> String {
    format!("file:///{}", path.display().to_string().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_is_first_non_empty_line_without_hashes() {
        assert_eq!(title_of("\n\n## Lista della spesa\nlatte"), "Lista della spesa");
        assert_eq!(title_of("   "), "");
    }

    #[test]
    fn preview_strips_markdown() {
        assert_eq!(preview_of("Titolo\n\n- [ ] **latte** fresco"), "latte fresco");
        assert_eq!(preview_of("Solo titolo"), "");
    }

    #[test]
    fn file_names_are_valid_on_windows() {
        assert_eq!(file_stem_for_title("Idee: app/web?"), "Idee- app-web-");
        assert_eq!(file_stem_for_title("Fine frase..."), "Fine frase");
        assert_eq!(file_stem_for_title("   "), untitled());
        assert_eq!(file_stem_for_title("con"), "con_");
        assert_eq!(file_stem_for_title("CON.txt"), "CON.txt_");
        assert_eq!(file_stem_for_title(&"a".repeat(300)).chars().count(), 100);
    }

    #[test]
    fn deleted_notes_can_be_restored() {
        let dir = tempdir();
        let p = dir.join("Da ripristinare.md");
        fs::write(&p, "ciao").unwrap();
        trash::delete(&p).unwrap();
        assert!(!p.exists());
        restore_from_trash(&p).unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "ciao");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn search_ignores_accents_and_case() {
        assert_eq!(fold("Città PERCHÉ"), "citta perche");
    }

    #[test]
    fn relative_links_work_across_folders() {
        let root = Path::new(r"C:\Note");
        let img = root.join("_allegati").join("a b.png");
        assert_eq!(relative_link(root, &img), "_allegati/a%20b.png");
        assert_eq!(relative_link(&root.join("Lavoro").join("X"), &img), "../../_allegati/a%20b.png");
        assert_eq!(resolve_link(&root.join("Lavoro"), "../_allegati/a%20b.png"), root.join("Lavoro").join(r"..\_allegati\a b.png"));
    }

    #[test]
    fn unique_path_skips_existing_files() {
        let dir = tempdir();
        fs::write(dir.join("Nota.md"), "x").unwrap();
        assert_eq!(unique_path(&dir, "Nota", None), dir.join("Nota 2.md"));
        // La nota stessa non è un conflitto, anche con maiuscole diverse.
        assert_eq!(unique_path(&dir, "Nota", Some(&dir.join("nota.md"))), dir.join("Nota.md"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn load_and_encode_round_trip_is_byte_identical() {
        let dir = tempdir();
        for bytes in [&b"\xEF\xBB\xBFciao\r\nmondo\r\n"[..], b"ciao\nmondo", "città\n".as_bytes()] {
            let p = dir.join("n.md");
            fs::write(&p, bytes).unwrap();
            let n = load_note(&p).unwrap();
            assert!(!n.text.contains('\r'));
            assert_eq!(encode(&n.text, n.crlf, n.bom), bytes);
        }
        fs::write(dir.join("bad.md"), [0xff, 0xfe, 0x00]).unwrap();
        assert!(load_note(&dir.join("bad.md")).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn scan_hides_dot_and_attachment_folders() {
        let dir = tempdir();
        fs::create_dir_all(dir.join("Lavoro")).unwrap();
        fs::create_dir_all(dir.join(".yonote")).unwrap();
        fs::create_dir_all(dir.join(ATTACHMENTS_DIR)).unwrap();
        fs::write(dir.join("Lavoro/Progetto.md"), "Progetto X\nnote").unwrap();
        fs::write(dir.join(".yonote/x.md"), "no").unwrap();
        fs::write(dir.join("immagine.png"), "no").unwrap();
        let v = Vault::open(dir.clone()).unwrap();
        assert_eq!(v.notes.len(), 1);
        assert_eq!(v.notes[0].title, "Progetto X");
        assert_eq!(v.tree.children.len(), 1);
        assert_eq!(v.titles, ["Progetto X"]);
        assert!(same_path(v.notes[0].path.parent().unwrap(), &dir.join("lavoro")));
        assert!(same_path(Path::new("C:/a/B.md"), Path::new(r"c:\a\b.md")));
        assert!(is_inside(Path::new(r"C:\Note\Lavoro\x.md"), Path::new("c:/note")));
        assert!(!is_inside(Path::new(r"C:\Note2\x.md"), Path::new(r"C:\Note")));
        assert!(!is_inside(Path::new(r"C:\Note"), Path::new(r"C:\Note")));
        fs::remove_dir_all(dir).unwrap();
    }

    fn tempdir() -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!("yonote-test-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&d).unwrap();
        d
    }
}
