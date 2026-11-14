//! Editor con anteprima live.
//!
//! È un unico `TextEdit` di egui: il testo resta Markdown puro, ma viene
//! disegnato formattato. La sintassi (`**`, `## `, `](url)`…) si vede solo
//! sulla riga dove si trova il cursore; sulle altre è nascosta. Checkbox,
//! puntini degli elenchi, citazioni, separatori e immagini sono disegnati
//! sopra il testo, così la selezione con il mouse funziona su tutta la nota.

use crate::i18n::tr;
use crate::markdown::{self, Fmt, Line, LineKind, Target};
use crate::{theme, vault};
use eframe::egui::{
    self, Color32, CursorIcon, Event, FontFamily, FontId, Id, Key, KeyboardShortcut, Modifiers, Pos2, Rect, Sense, Stroke, TextureOptions,
    Vec2,
    load::{SizeHint, SizedTexture, TexturePoll},
    text::{ByteIndex, CCursor, CCursorRange, LayoutJob, LayoutSection, TextFormat},
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Cosa l'editor chiede all'app di fare.
pub enum EditorAction {
    OpenUrl(String),
    OpenNote(String),
    FilterTag(String),
    Error(String),
    /// Azioni sul file della nota, dal menu del tasto destro.
    TogglePin,
    RevealInExplorer,
    CopyPath,
    Delete,
}

/// Comandi del menu del tasto destro.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MenuCmd {
    Cut,
    Copy,
    Paste,
    SelectAll,
    Wrap(&'static str, &'static str),
    Link,
    Block(Block),
    InsertImage,
    InsertNoteLink,
    InsertRule,
    /// Trova (false) o Trova e sostituisci (true).
    Find(bool),
    File(FileCmd),
}

/// Formato di paragrafo da applicare alle righe selezionate.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Block {
    /// Titolo di livello 1–3; 0 = testo normale.
    Heading(u8),
    Task,
    Bullet,
    Numbered,
    Quote,
}

/// Lunghezza del prefisso di blocco della riga (`## `, `- [ ] `, `1. `, `> `…) e il suo tipo.
fn block_of(line: &str) -> (Option<Block>, usize) {
    let hashes = line.bytes().take_while(|&c| c == b'#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        return (Some(Block::Heading(hashes.min(3) as u8)), hashes + 1);
    }
    match markdown::list_continuation(line) {
        Some((LineKind::Task { .. }, len, _)) => (Some(Block::Task), len),
        Some((LineKind::Bullet, len, _)) => (Some(Block::Bullet), len),
        Some((LineKind::Ordered, len, _)) => (Some(Block::Numbered), len),
        Some((LineKind::Quote, len, _)) => (Some(Block::Quote), len),
        _ => (None, 0),
    }
}

/// Applica `block` a tutte le righe toccate dalla selezione `a..b` (in byte).
/// Gli elenchi numerati vengono numerati 1, 2, 3…; le righe vuote restano vuote.
/// Se tutte le righe hanno già quel formato (elenchi, checklist, citazioni), lo toglie.
/// Restituisce il nuovo testo e la nuova selezione.
pub fn apply_block(text: &str, a: usize, b: usize, block: Block) -> (String, usize, usize) {
    let first = text[..a].rfind('\n').map_or(0, |i| i + 1);
    // Una selezione che finisce all'inizio di una riga non comprende quella riga.
    let b_eff = if b > a && b > first && text[..b].ends_with('\n') { b - 1 } else { b };
    let last_end = text[b_eff..].find('\n').map_or(text.len(), |i| b_eff + i);
    let lines: Vec<&str> = text[first..last_end].split('\n').collect();

    let is_list = !matches!(block, Block::Heading(_));
    let remove = is_list && lines.iter().filter(|l| !l.trim().is_empty()).all(|l| block_of(l).0 == Some(block));

    let mut out = Vec::with_capacity(lines.len());
    let mut number = 0;
    for line in &lines {
        if line.trim().is_empty() {
            out.push((*line).to_owned());
            continue;
        }
        let (_, len) = block_of(line);
        let indent: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        // Il testo della riga senza il vecchio prefisso (né rientro, se non ne aveva uno).
        let body = if len > 0 { &line[len..] } else { line.trim_start_matches([' ', '\t']) };
        let prefix = match block {
            _ if remove => String::new(),
            Block::Heading(0) => String::new(),
            Block::Heading(n) => format!("{} ", "#".repeat(n as usize)),
            Block::Task => format!("{indent}- [ ] "),
            Block::Bullet => format!("{indent}- "),
            Block::Numbered => {
                number += 1;
                format!("{indent}{number}. ")
            }
            Block::Quote => "> ".to_owned(),
        };
        out.push(format!("{prefix}{body}"));
    }
    let new_block = out.join("\n");
    let new_text = format!("{}{}{}", &text[..first], new_block, &text[last_end..]);
    let (na, nb) = if a == b {
        // Cursore senza selezione: resta in fondo alla riga modificata.
        let end = first + new_block.len();
        (end, end)
    } else {
        (first, first + new_block.len())
    };
    (new_text, na, nb)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FileCmd {
    Pin,
    Reveal,
    CopyPath,
    Delete,
}

/// Informazioni sul resto del vault che servono all'editor.
pub struct EditorCtx<'a> {
    /// Cartella della nota aperta (per i percorsi delle immagini).
    pub note_dir: &'a Path,
    pub vault_root: &'a Path,
    pub titles: &'a [String],
    pub tags: &'a [String],
    pub font_size: f32,
    pub line_spacing: f32,
    pub width: f32,
    /// La nota ha già un file su disco (le note nuove non ancora salvate no).
    pub has_file: bool,
    /// Falso per i file aperti da fuori dalla cartella delle note.
    pub can_pin: bool,
    pub pinned: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SuggestKind {
    Note,
    Tag,
}

/// Menu dei suggerimenti per `[[nota` e `#tag`.
#[derive(Clone, Debug)]
struct Suggest {
    kind: SuggestKind,
    /// Byte da cui inizia la parola cercata (dopo `[[` o `#`).
    start: usize,
    items: Vec<String>,
    selected: usize,
    pos: Pos2,
}

#[derive(Default)]
pub struct Editor {
    /// Cursore del fotogramma precedente (indici in caratteri).
    cursor: Option<CCursorRange>,
    focus_next_frame: bool,
    suggest: Option<Suggest>,
    /// Posizione di un suggerimento chiuso con Esc (non riaprirlo lì).
    dismissed: Option<usize>,
    /// Selezione (in byte) al momento del clic destro: il menu agisce su questa,
    /// anche se cliccando sulla voce l'editor perde il focus.
    menu_range: Option<(usize, usize)>,
    /// Selezione da rimettere quando l'editor riprende il focus dopo un comando del menu
    /// (riprendendo il focus, egui la azzererebbe).
    restore_range: Option<(usize, usize)>,
    /// Righe analizzate dell'ultimo testo visto (si rifà l'analisi solo se il testo cambia).
    parsed: (u64, Arc<Vec<Line>>),
    /// Ultima impaginazione e la chiave da cui dipende.
    layout: (u64, Option<LayoutJob>),
    /// Trova e sostituisci.
    pub find: crate::find::Find,
}

fn hash_of(value: impl std::hash::Hash) -> u64 {
    use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};
    BuildHasherDefault::<DefaultHasher>::default().hash_one(value)
}

/// Analizza `text` solo se è diverso dall'ultima volta.
fn parse_cached(cache: &mut (u64, Arc<Vec<Line>>), text: &str) -> Arc<Vec<Line>> {
    let h = hash_of(text);
    if cache.0 != h || (cache.1.is_empty() && !text.is_empty()) {
        *cache = (h, Arc::new(markdown::parse(text)));
    }
    cache.1.clone()
}

pub struct EditorResult {
    pub changed: bool,
    pub action: Option<EditorAction>,
}

/// Dimensione dei caratteri nascosti (sintassi fuori dalla riga del cursore):
/// così piccoli da non lasciare spazi visibili, ma ancora selezionabili.
const HIDDEN_SIZE: f32 = 0.05;

/// Dimensioni del testo per tipo di riga, rispetto alla dimensione base.
fn scale(kind: LineKind) -> f32 {
    match kind {
        LineKind::Title => 1.85,
        LineKind::Heading(1) => 1.5,
        LineKind::Heading(2) => 1.28,
        LineKind::Heading(_) => 1.12,
        LineKind::Code | LineKind::CodeFence => 0.9,
        _ => 1.0,
    }
}

fn editor_id() -> Id {
    Id::unique("yonote-editor")
}

fn char_to_byte(text: &str, char_idx: usize) -> usize {
    text.char_indices().nth(char_idx).map_or(text.len(), |(b, _)| b)
}

fn byte_to_char(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].chars().count()
}

fn line_bounds(text: &str, byte: usize) -> (usize, usize) {
    let start = text[..byte].rfind('\n').map_or(0, |i| i + 1);
    let end = text[byte..].find('\n').map_or(text.len(), |i| byte + i);
    (start, end)
}

/// Altezza a cui mostrare un'immagine larga al massimo `width`.
fn image_display_size(size: Vec2, width: f32) -> Vec2 {
    let w = size.x.min(width).max(1.0);
    let mut s = Vec2::new(w, size.y * w / size.x.max(1.0));
    if s.y > 460.0 {
        s *= 460.0 / s.y;
    }
    s
}

impl Editor {
    pub fn request_focus(&mut self) {
        self.focus_next_frame = true;
    }

    /// Da chiamare quando si apre un'altra nota.
    pub fn reset(&mut self) {
        self.cursor = None;
        self.suggest = None;
        self.dismissed = None;
    }

    pub fn has_selection(&self) -> bool {
        self.cursor.is_some_and(|c| c.primary.index != c.secondary.index)
    }

    /// Selezione attuale in byte (o il cursore), se l'editor ne ha una.
    fn selection_bytes(&self, text: &str) -> Option<(usize, usize)> {
        let c = self.cursor?;
        let (a, b) = (c.primary.index.0.min(c.secondary.index.0), c.primary.index.0.max(c.secondary.index.0));
        Some((char_to_byte(text, a), char_to_byte(text, b)))
    }

    /// Ctrl+F / Ctrl+H: apre la barra; il testo selezionato diventa la ricerca.
    pub fn open_find(&mut self, replace: bool, text: &str) {
        let sel = self.selection_bytes(text);
        self.find.show(replace, sel.map(|(a, b)| &text[a..b]), sel.map_or(0, |(a, _)| a));
    }

    /// Barra Trova/Sostituisci; restituisce true se ha modificato il testo.
    pub fn find_bar(&mut self, ctx: &egui::Context, right_top: Pos2, text: &mut String) -> bool {
        match self.find.bar(ctx, right_top, text) {
            crate::find::BarOutcome::None => false,
            crate::find::BarOutcome::Changed => true,
            crate::find::BarOutcome::Closed(active) => {
                // Si torna a scrivere dove si era arrivati con la ricerca.
                if let Some(r) = active {
                    self.restore_range = Some((r.start, r.end));
                }
                self.focus_next_frame = true;
                false
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, text: &mut String, cx: &EditorCtx<'_>) -> EditorResult {
        let id = editor_id();
        let mut changed = false;
        let mut action = None;
        let focused = ui.memory(|m| m.has_focus(id));

        if focused {
            changed |= self.suggest_keys(ui, text);
            changed |= self.shortcuts(ui, text);
            match self.paste_images(ui, text, cx) {
                Ok(c) => changed |= c,
                Err(e) => action = Some(EditorAction::Error(e)),
            }
        }
        match self.dropped_files(ui, text, cx) {
            Ok(c) => changed |= c,
            Err(e) => action = Some(EditorAction::Error(e)),
        }

        // Immagini: dimensioni note prima dell'impaginazione.
        let mut images: HashMap<String, (SizedTexture, Vec2)> = HashMap::new();
        for line in parse_cached(&mut self.parsed, text).iter().filter(|l| l.kind == LineKind::Image) {
            let Some(src) = &line.src else { continue };
            let path = vault::resolve_link(cx.note_dir, src);
            let uri = vault::file_uri(&path);
            if let Ok(TexturePoll::Ready { texture }) = ui.ctx().try_load_texture(&uri, TextureOptions::LINEAR, SizeHint::default()) {
                images.insert(src.clone(), (texture, image_display_size(texture.size, cx.width)));
            }
        }

        let visuals = ui.visuals().clone();
        let cursor_line = self.cursor.map(|c| {
            let byte = char_to_byte(text, c.primary.index.0);
            text[..byte].matches('\n').count()
        });
        let (font_size, spacing) = (cx.font_size, cx.line_spacing);
        let sizes: HashMap<String, f32> = images.iter().map(|(k, v)| (k.clone(), v.1.y)).collect();
        let mut sorted_sizes: Vec<(&String, u32)> = sizes.iter().map(|(k, v)| (k, v.to_bits())).collect();
        sorted_sizes.sort();
        // Tutto ciò da cui dipende l'aspetto del testo, tranne il testo stesso.
        let style_key = hash_of((
            cursor_line,
            font_size.to_bits(),
            spacing.to_bits(),
            &sorted_sizes,
            visuals.text_color(),
            visuals.hyperlink_color,
            visuals.dark_mode,
        ));
        let (parsed, layout_cache) = (&mut self.parsed, &mut self.layout);
        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| -> Arc<egui::Galley> {
            let text = buf.as_str();
            let lines = parse_cached(parsed, text);
            let key = hash_of((style_key, parsed.0));
            let mut job = match &layout_cache.1 {
                Some(job) if layout_cache.0 == key => job.clone(),
                _ => {
                    let job = layout_job(text, &lines, cursor_line, font_size, spacing, &visuals, &sizes);
                    *layout_cache = (key, Some(job.clone()));
                    job
                }
            };
            job.wrap.max_width = wrap_width;
            ui.fonts_mut(|f| f.layout_job(job))
        };

        // Posto per l'evidenziazione della ricerca, sotto il testo (si riempie dopo l'impaginazione).
        let find_slot = ui.painter().add(egui::Shape::Noop);
        let output = egui::TextEdit::multiline(text)
            .id(id)
            .frame(egui::Frame::NONE)
            .desired_width(cx.width)
            .desired_rows(18)
            .lock_focus(true)
            .hint_text(tr("Inizia a scrivere… la prima riga diventa il titolo.", "Start typing… the first line becomes the title."))
            .layouter(&mut layouter)
            .show(ui);

        if self.focus_next_frame {
            output.response.request_focus();
            self.focus_next_frame = false;
        }
        if output.response.has_focus()
            && let Some((a, b)) = self.restore_range.take()
        {
            let (a, b) = (a.min(text.len()), b.min(text.len()));
            if text.is_char_boundary(a) && text.is_char_boundary(b) {
                set_cursor(ui, text, a, b);
            }
        }
        changed |= output.response.changed();
        let mut new_cursor = if output.response.has_focus() { output.cursor_range } else { None };
        // Il clic destro non deve perdere la selezione: serve al menu (Copia, Grassetto…).
        let right_click = ui.input(|i| i.pointer.secondary_pressed()) && output.response.hovered();
        if right_click {
            if let Some(prev) = self.cursor.filter(|c| c.primary.index != c.secondary.index) {
                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) {
                    state.cursor.set_char_range(Some(prev));
                    egui::TextEdit::store_state(ui.ctx(), id, state);
                }
                new_cursor = Some(prev);
            }
            self.menu_range = cursor_bytes(ui, text);
        }
        // Selezione trascinata oltre il bordo visibile: la nota scorre (più veloce quanto più si è lontani).
        if output.response.dragged()
            && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
        {
            let visible = ui.clip_rect();
            let edge = 24.0;
            let over = if pos.y < visible.top() + edge {
                visible.top() + edge - pos.y
            } else if pos.y > visible.bottom() - edge {
                visible.bottom() - edge - pos.y
            } else {
                0.0
            };
            if over != 0.0 {
                let speed = (over.abs() * 0.35).clamp(3.0, 60.0) * over.signum();
                ui.scroll_with_delta(Vec2::new(0.0, speed));
                ui.ctx().request_repaint();
            }
        }
        if new_cursor.map(|c| c.primary.index) != self.cursor.map(|c| c.primary.index) {
            ui.ctx().request_repaint(); // la riga attiva cambia aspetto
        }
        self.cursor = new_cursor;

        // Decorazioni disegnate sopra il testo (stessa analisi usata per l'impaginazione).
        let lines = parse_cached(&mut self.parsed, text);
        let galley = &output.galley;
        let origin = output.galley_pos;
        let painter = ui.painter_at(output.text_clip_rect.expand(60.0));
        let pal = theme::palette(ui.ctx());
        let row_rect = |char_idx: usize| galley.pos_from_cursor(CCursor::new(char_idx)).translate(origin.to_vec2());

        let mut char_start = 0;
        let mut toggle: Option<usize> = None;
        for (n, line) in lines.iter().enumerate() {
            let line_text = &text[line.range.clone()];
            let line_chars = line_text.chars().count();
            let indent_bytes = line_text.len() - line_text.trim_start().len();
            let indent_chars = line_text[..indent_bytes].chars().count();
            let first = row_rect(char_start + indent_chars);
            let is_cursor_line = cursor_line == Some(n);
            let size = font_size * scale(line.kind);
            match line.kind {
                LineKind::Bullet => {
                    let c = Pos2::new(first.min.x + list_indent(line_text, size) + size * 0.42, first.center().y);
                    painter.circle_filled(c, size * 0.13, pal.text_muted);
                }
                LineKind::Task { checked } => {
                    let side = (size * 0.98).round();
                    let x = first.min.x + list_indent(line_text, size) + side * 0.55;
                    let r = Rect::from_center_size(Pos2::new(x, first.center().y), Vec2::splat(side));
                    let resp = ui.interact(r.expand(3.0), id.with(("task", n)), Sense::click());
                    let label: String =
                        line_text.trim_start().get(6..).unwrap_or("").chars().filter(|c| !matches!(c, '*' | '_' | '~' | '`')).collect();
                    crate::widgets::a11y(&resp, egui::Role::CheckBox, label.trim(), Some(checked));
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    if checked {
                        painter.circle_filled(r.center(), side / 2.0, pal.accent);
                        let s = Stroke::new(1.8, pal.on_accent);
                        let p = |x: f32, y: f32| Pos2::new(r.min.x + x * side, r.min.y + y * side);
                        painter.line_segment([p(0.28, 0.52), p(0.44, 0.67)], s);
                        painter.line_segment([p(0.44, 0.67), p(0.73, 0.36)], s);
                    } else {
                        let color = if resp.hovered() { pal.accent_text } else { pal.text_muted };
                        painter.circle_stroke(r.center(), side / 2.0 - 0.75, Stroke::new(1.5, color));
                    }
                    if resp.clicked() {
                        toggle = Some(line.range.start + indent_bytes + 3);
                    }
                }
                LineKind::Quote => {
                    let last = row_rect(char_start + line_chars);
                    let x = first.min.x + 1.5;
                    painter.line_segment(
                        [Pos2::new(x, first.min.y + 3.0), Pos2::new(x, last.max.y - 3.0)],
                        Stroke::new(3.0, pal.accent.gamma_multiply(0.8)),
                    );
                }
                LineKind::Rule if !is_cursor_line => {
                    let y = first.center().y;
                    let x0 = output.text_clip_rect.min.x;
                    painter.line_segment([Pos2::new(x0, y), Pos2::new(x0 + cx.width, y)], Stroke::new(1.0, pal.border.gamma_multiply(2.0)));
                }
                LineKind::Image if !is_cursor_line => {
                    if let Some((tex, sz)) = line.src.as_ref().and_then(|s| images.get(s)) {
                        let r = Rect::from_min_size(Pos2::new(output.text_clip_rect.min.x, first.min.y + 6.0), *sz);
                        egui::Image::from_texture(*tex).corner_radius(10).paint_at(ui, r);
                        let resp = ui.interact(r, id.with(("img", n)), Sense::click());
                        if resp.double_clicked() {
                            let path = vault::resolve_link(cx.note_dir, line.src.as_deref().unwrap_or(""));
                            crate::platform::reveal_file(&path);
                        }
                        resp.on_hover_text(tr("Doppio clic per aprire l'immagine", "Double-click to open the image"));
                    } else {
                        let r =
                            Rect::from_min_size(Pos2::new(output.text_clip_rect.min.x, first.min.y), Vec2::new(cx.width, first.height()));
                        painter.text(
                            r.left_center(),
                            egui::Align2::LEFT_CENTER,
                            format!("⧉  {}", line.src.as_deref().unwrap_or("")),
                            FontId::proportional(font_size * 0.85),
                            pal.text_muted,
                        );
                    }
                }
                _ => {}
            }
            char_start += line_chars + 1;
        }

        // Corrispondenze della ricerca, sul testo di questo fotogramma.
        if self.find.open {
            self.find.refresh(text, None);
            let rect_of = |a: usize, b: usize| -> Vec<Rect> {
                let mut rects: Vec<Rect> = Vec::new();
                for i in a..b {
                    let (l, r) = (row_rect(i), row_rect(i + 1));
                    if (l.min.y - r.min.y).abs() > 0.5 {
                        continue; // fine riga: il carattere va a capo
                    }
                    let ch = Rect::from_min_max(l.min, Pos2::new(r.min.x, l.max.y));
                    match rects.last_mut() {
                        Some(last) if (last.min.y - ch.min.y).abs() < 0.5 => *last = last.union(ch),
                        _ => rects.push(ch),
                    }
                }
                rects
            };
            let (shapes, active) = self.find.highlights(text, ui.clip_rect(), pal.dark, rect_of);
            ui.painter().set(find_slot, egui::Shape::Vec(shapes));
            if self.find.scroll {
                self.find.scroll = false;
                if let Some(r) = active {
                    ui.scroll_to_rect(r.expand(40.0), Some(egui::Align::Center));
                }
            }
        }

        if let Some(pos) = toggle {
            let checked = text.as_bytes().get(pos).is_some_and(|b| *b == b'x' || *b == b'X');
            text.replace_range(pos..pos + 1, if checked { " " } else { "x" });
            changed = true;
        }

        // Ctrl+clic su link, link a note e tag.
        let ctrl = ui.input(|i| i.modifiers.command);
        if let Some(pos) = output.response.hover_pos() {
            let cc = galley.cursor_from_pos(pos - origin);
            if let Some(target) = target_at(&lines, char_to_byte(text, cc.index.0)) {
                if ctrl {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    if output.response.clicked() {
                        action = Some(match target {
                            Target::Url(u) => EditorAction::OpenUrl(u),
                            Target::Note(n) => EditorAction::OpenNote(n),
                            Target::Tag(t) => EditorAction::FilterTag(t),
                        });
                    }
                } else {
                    output.response.response.clone().on_hover_text_at_pointer(tr("Ctrl+clic per aprire", "Ctrl+click to open"));
                }
            }
        }

        // Menu del tasto destro: modifica, formato, inserimento e azioni sul file.
        let mut menu_cmd = None;
        let menu_range = self.menu_range;
        output.response.response.context_menu(|ui| {
            menu_cmd = context_menu(ui, cx, menu_range.is_some_and(|(a, b)| a != b));
        });
        if let Some(cmd) = menu_cmd {
            match self.run_menu(ui, text, cx, cmd) {
                Ok((c, a)) => {
                    changed |= c;
                    if a.is_some() {
                        action = a;
                    }
                }
                Err(e) => action = Some(EditorAction::Error(e)),
            }
        }

        // Suggerimenti per [[nota e #tag.
        self.update_suggest(text, cx, row_rect);
        if let Some(chosen) = self.suggest_popup(ui) {
            changed |= self.accept_suggestion(ui, text, chosen);
        }

        EditorResult { changed, action }
    }

    // -----------------------------------------------------------------------
    // Suggerimenti

    fn update_suggest(&mut self, text: &str, cx: &EditorCtx<'_>, row_rect: impl Fn(usize) -> Rect) {
        let Some(c) = self.cursor.filter(|c| c.primary.index == c.secondary.index) else {
            self.suggest = None;
            return;
        };
        let byte = char_to_byte(text, c.primary.index.0);
        let (line_start, _) = line_bounds(text, byte);
        let before = &text[line_start..byte];
        let found = if let Some(open) = before.rfind("[[").filter(|&o| !before[o..].contains("]]")) {
            Some((SuggestKind::Note, line_start + open + 2))
        } else {
            let word_len: usize =
                before.chars().rev().take_while(|&c| c.is_alphanumeric() || matches!(c, '_' | '-' | '/')).map(char::len_utf8).sum();
            let hash = before.len() - word_len;
            (word_len > 0
                && before[..hash].ends_with('#')
                && before[..hash - 1].chars().next_back().is_none_or(char::is_whitespace)
                && !before.trim_start().starts_with("# "))
            .then_some((SuggestKind::Tag, line_start + hash))
        };
        let Some((kind, start)) = found else {
            self.suggest = None;
            self.dismissed = None;
            return;
        };
        if self.dismissed == Some(start) {
            self.suggest = None;
            return;
        }
        let query = &text[start..byte];
        let source = if kind == SuggestKind::Note { cx.titles } else { cx.tags };
        let mut items = vault::fuzzy_rank(query, source, 8);
        if kind == SuggestKind::Tag {
            items.retain(|t| *t != query.to_lowercase());
        }
        if items.is_empty() {
            self.suggest = None;
            return;
        }
        let r = row_rect(byte_to_char(text, start));
        let selected = self.suggest.as_ref().filter(|s| s.start == start && s.kind == kind).map_or(0, |s| s.selected).min(items.len() - 1);
        self.suggest = Some(Suggest { kind, start, items, selected, pos: r.left_bottom() + Vec2::new(-8.0, 6.0) });
    }

    /// Frecce, Invio, Tab ed Esc quando il menu dei suggerimenti è aperto.
    fn suggest_keys(&mut self, ui: &egui::Ui, text: &mut String) -> bool {
        let Some(s) = &mut self.suggest else { return false };
        let n = s.items.len();
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
            s.selected = (s.selected + 1) % n;
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
            s.selected = (s.selected + n - 1) % n;
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.dismissed = Some(s.start);
            self.suggest = None;
            return false;
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter) || i.consume_key(Modifiers::NONE, Key::Tab)) {
            let chosen = s.items[s.selected].clone();
            return self.accept_suggestion(ui, text, chosen);
        }
        false
    }

    fn accept_suggestion(&mut self, ui: &egui::Ui, text: &mut String, chosen: String) -> bool {
        let (Some(s), Some(c)) = (self.suggest.take(), self.cursor) else { return false };
        let end = char_to_byte(text, c.primary.index.0);
        let insert = match s.kind {
            SuggestKind::Note if text[end..].starts_with("]]") => chosen,
            SuggestKind::Note => format!("{chosen}]]"),
            SuggestKind::Tag => format!("{chosen} "),
        };
        let mut after = s.start + insert.len();
        if s.kind == SuggestKind::Note && text[end..].starts_with("]]") {
            after += 2;
        }
        text.replace_range(s.start..end, &insert);
        set_cursor(ui, text, after, after);
        self.focus_next_frame = true;
        true
    }

    fn suggest_popup(&mut self, ui: &egui::Ui) -> Option<String> {
        let s = self.suggest.as_ref()?;
        let pal = theme::palette(ui.ctx());
        let mut chosen = None;
        egui::Area::new(Id::unique("yonote-suggest")).order(egui::Order::Foreground).fixed_pos(s.pos).show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).inner_margin(6).show(ui, |ui| {
                ui.set_min_width(220.0);
                let icon = if s.kind == SuggestKind::Note { crate::icons::Icon::Notes } else { crate::icons::Icon::Hash };
                for (i, item) in s.items.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(220.0), 30.0), Sense::click());
                    if i == s.selected {
                        ui.painter().rect_filled(rect, 6.0, pal.accent_fill);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(rect, 6.0, pal.hover);
                    }
                    crate::icons::paint(
                        ui,
                        icon,
                        Rect::from_center_size(rect.left_center() + Vec2::new(16.0, 0.0), Vec2::splat(15.0)),
                        pal.text_muted,
                    );
                    ui.painter().text(
                        rect.left_center() + Vec2::new(32.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        item,
                        FontId::proportional(14.0),
                        pal.text,
                    );
                    if resp.clicked() {
                        chosen = Some(item.clone());
                    }
                }
            });
        });
        chosen
    }

    // -----------------------------------------------------------------------
    // Scorciatoie e comportamenti di scrittura

    fn shortcuts(&mut self, ui: &egui::Ui, text: &mut String) -> bool {
        let Some((a, b)) = cursor_bytes(ui, text) else { return false };
        let cmd = Modifiers::COMMAND;
        let pressed = |m: Modifiers, k: Key| ui.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k)));

        // Grassetto, corsivo, barrato, link: avvolgono la selezione.
        for (m, k, open, close) in [
            (cmd, Key::B, "**", "**"),
            (cmd, Key::I, "*", "*"),
            (cmd | Modifiers::SHIFT, Key::X, "~~", "~~"),
            (cmd | Modifiers::SHIFT, Key::C, "`", "`"),
        ] {
            if pressed(m, k) {
                return wrap_selection(ui, text, a, b, open, close);
            }
        }
        if a != b && pressed(cmd, Key::K) {
            text.insert_str(b, "]()");
            text.insert(a, '[');
            set_cursor(ui, text, b + 3, b + 3);
            return true;
        }

        // Titoli e checklist: valgono per tutte le righe selezionate.
        for (k, block) in [
            (Key::Num1, Block::Heading(1)),
            (Key::Num2, Block::Heading(2)),
            (Key::Num3, Block::Heading(3)),
            (Key::Num0, Block::Heading(0)),
            (Key::L, Block::Task),
        ] {
            if pressed(cmd, k) {
                set_block(ui, text, a, b, block);
                return true;
            }
        }
        let (start, end) = line_bounds(text, a);
        let line = text[start..end].to_owned();
        let cont = markdown::list_continuation(&line);
        if pressed(cmd, Key::Enter)
            && let Some((LineKind::Task { checked }, _, _)) = cont
        {
            let indent = line.len() - line.trim_start().len();
            let pos = start + indent + 3;
            text.replace_range(pos..pos + 1, if checked { " " } else { "x" });
            return true;
        }

        // Invio negli elenchi: continua l'elenco o, su una voce vuota, lo chiude.
        if a == b
            && let Some((_, len, next)) = &cont
            && a >= start + len
            && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
        {
            if line[*len..].trim().is_empty() {
                return replace_prefix(ui, text, start, *len, "", a);
            }
            let insert = format!("\n{next}");
            text.insert_str(a, &insert);
            set_cursor(ui, text, a + insert.len(), a + insert.len());
            return true;
        }

        // Tab e Maiusc+Tab negli elenchi: rientro.
        if let Some((kind, ..)) = cont
            && kind != LineKind::Quote
        {
            if ui.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Tab)) {
                let spaces = line.bytes().take_while(|&c| c == b' ').count().min(2);
                if spaces > 0 {
                    text.replace_range(start..start + spaces, "");
                    let (na, nb) = (a.saturating_sub(spaces).max(start), b.saturating_sub(spaces).max(start));
                    set_cursor(ui, text, na, nb);
                    return true;
                }
                return false;
            }
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab)) {
                text.insert_str(start, "  ");
                set_cursor(ui, text, a + 2, b + 2);
                return true;
            }
        }
        false
    }

    /// Ctrl+V con un'immagine negli appunti: la salva in `_allegati` e la inserisce.
    fn paste_images(&mut self, ui: &egui::Ui, text: &mut String, cx: &EditorCtx<'_>) -> Result<bool, String> {
        let images: Vec<Arc<egui::ColorImage>> = ui.input_mut(|i| {
            let mut out = Vec::new();
            i.events.retain(|e| match e {
                Event::PasteImage(img) => {
                    out.push(img.clone());
                    false
                }
                _ => true,
            });
            out
        });
        let mut changed = false;
        for img in images {
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
            let path = save_png(cx, img.size[0], img.size[1], bytes)?;
            changed |= self.insert_image_link(ui, text, cx, &path);
        }
        Ok(changed)
    }

    /// Esegue un comando scelto dal menu del tasto destro.
    fn run_menu(
        &mut self,
        ui: &egui::Ui,
        text: &mut String,
        cx: &EditorCtx<'_>,
        cmd: MenuCmd,
    ) -> Result<(bool, Option<EditorAction>), String> {
        if let MenuCmd::Find(replace) = cmd {
            let range = self.menu_range.take();
            self.find.show(replace, range.and_then(|(a, b)| text.get(a..b)), range.map_or(0, |(a, _)| a));
            return Ok((false, None));
        }
        self.focus_next_frame = true;
        let (a, b) = self
            .menu_range
            .take()
            .filter(|&(a, b)| b <= text.len() && text.is_char_boundary(a) && text.is_char_boundary(b))
            .or_else(|| cursor_bytes(ui, text))
            .unwrap_or((text.len(), text.len()));
        // I comandi che usano il cursore partono dalla selezione salvata.
        set_cursor(ui, text, a, b);
        let changed = match cmd {
            MenuCmd::Copy | MenuCmd::Cut => {
                if a == b {
                    return Ok((false, None));
                }
                ui.ctx().copy_text(text[a..b].to_owned());
                if cmd == MenuCmd::Cut {
                    text.replace_range(a..b, "");
                    set_cursor(ui, text, a, a);
                }
                cmd == MenuCmd::Cut
            }
            MenuCmd::Paste => {
                let mut clip = arboard::Clipboard::new().map_err(|e| e.to_string())?;
                if let Ok(t) = clip
                    .get_text()
                    .map(|t| t.replace("\r\n", "\n"))
                    .and_then(|t| if t.is_empty() { Err(arboard::Error::ContentNotAvailable) } else { Ok(t) })
                {
                    text.replace_range(a..b, &t);
                    set_cursor(ui, text, a + t.len(), a + t.len());
                    true
                } else if let Ok(img) = clip.get_image() {
                    let path = save_png(cx, img.width, img.height, img.bytes.into_owned())?;
                    self.insert_image_link(ui, text, cx, &path)
                } else {
                    false
                }
            }
            MenuCmd::SelectAll => {
                set_cursor(ui, text, 0, text.len());
                false
            }
            MenuCmd::Wrap(open, close) => wrap_selection(ui, text, a, b, open, close),
            MenuCmd::Link => {
                text.insert_str(b, "]()");
                text.insert(a, '[');
                // Senza selezione il cursore va tra le parentesi quadre, altrimenti tra le tonde.
                let pos = if a == b { a + 1 } else { b + 3 };
                set_cursor(ui, text, pos, pos);
                true
            }
            MenuCmd::Block(block) => {
                set_block(ui, text, a, b, block);
                self.restore_range = cursor_bytes(ui, text);
                true
            }
            MenuCmd::InsertNoteLink => {
                text.replace_range(a..b, "[[");
                set_cursor(ui, text, a + 2, a + 2);
                true
            }
            MenuCmd::InsertRule => {
                let (start, _) = line_bounds(text, a);
                let insert = if a > start { "\n---\n" } else { "---\n" };
                text.insert_str(a, insert);
                set_cursor(ui, text, a + insert.len(), a + insert.len());
                true
            }
            MenuCmd::InsertImage => {
                let Some(files) = rfd::FileDialog::new()
                    .set_title(tr("Scegli un'immagine", "Choose an image"))
                    .add_filter(tr("Immagini", "Images"), &["png", "jpg", "jpeg", "gif", "webp"])
                    .pick_files()
                else {
                    return Ok((false, None));
                };
                let mut changed = false;
                for f in files {
                    let path = import_image(cx, &f)?;
                    changed |= self.insert_image_link(ui, text, cx, &path);
                }
                changed
            }
            MenuCmd::Find(_) => false,
            MenuCmd::File(f) => {
                let action = match f {
                    FileCmd::Pin => EditorAction::TogglePin,
                    FileCmd::Reveal => EditorAction::RevealInExplorer,
                    FileCmd::CopyPath => EditorAction::CopyPath,
                    FileCmd::Delete => EditorAction::Delete,
                };
                return Ok((false, Some(action)));
            }
        };
        Ok((changed, None))
    }

    /// File trascinati nella finestra: le immagini vengono copiate negli allegati.
    fn dropped_files(&mut self, ui: &egui::Ui, text: &mut String, cx: &EditorCtx<'_>) -> Result<bool, String> {
        let files: Vec<PathBuf> = ui.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        let mut changed = false;
        for f in files {
            let ext = f.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp") {
                continue;
            }
            let path = import_image(cx, &f)?;
            if self.cursor.is_none() {
                // Senza cursore: in fondo alla nota.
                let end = text.len();
                set_cursor(ui, text, end, end);
                self.cursor = Some(CCursorRange::one(CCursor::new(text.chars().count())));
            }
            changed |= self.insert_image_link(ui, text, cx, &path);
        }
        Ok(changed)
    }

    fn insert_image_link(&mut self, ui: &egui::Ui, text: &mut String, cx: &EditorCtx<'_>, path: &Path) -> bool {
        let link = vault::relative_link(cx.note_dir, path);
        let (a, b) = cursor_bytes(ui, text).unwrap_or((text.len(), text.len()));
        let (line_start, line_end) = line_bounds(text, a);
        // L'immagine va su una riga tutta sua.
        let before = if a > line_start { "\n" } else { "" };
        // Dopo l'immagine si continua sempre su una riga nuova.
        let _ = line_end;
        let after = "\n";
        let insert = format!("{before}![]({link}){after}");
        text.replace_range(a..b, &insert);
        let pos = a + insert.len();
        set_cursor(ui, text, pos, pos);
        true
    }
}

// ---------------------------------------------------------------------------
// Menu del tasto destro

fn item(ui: &mut egui::Ui, label: &str, shortcut: &str, enabled: bool) -> bool {
    let mut b = egui::Button::new(label);
    if !shortcut.is_empty() {
        b = b.shortcut_text(shortcut);
    }
    ui.add_enabled(enabled, b).clicked()
}

fn context_menu(ui: &mut egui::Ui, cx: &EditorCtx<'_>, has_selection: bool) -> Option<MenuCmd> {
    ui.set_min_width(240.0);
    let mut cmd = None;
    if item(ui, tr("Taglia", "Cut"), "Ctrl+X", has_selection) {
        cmd = Some(MenuCmd::Cut);
    }
    if item(ui, tr("Copia", "Copy"), "Ctrl+C", has_selection) {
        cmd = Some(MenuCmd::Copy);
    }
    if item(ui, tr("Incolla", "Paste"), "Ctrl+V", true) {
        cmd = Some(MenuCmd::Paste);
    }
    if item(ui, tr("Seleziona tutto", "Select all"), "Ctrl+A", true) {
        cmd = Some(MenuCmd::SelectAll);
    }
    if item(ui, tr("Trova…", "Find…"), "Ctrl+F", true) {
        cmd = Some(MenuCmd::Find(false));
    }
    if item(ui, tr("Sostituisci…", "Replace…"), "Ctrl+H", true) {
        cmd = Some(MenuCmd::Find(true));
    }
    ui.separator();
    ui.menu_button(tr("Formato", "Format"), |ui| {
        ui.set_min_width(210.0);
        for (label, sc, open, close) in [
            (tr("Grassetto", "Bold"), "Ctrl+B", "**", "**"),
            (tr("Corsivo", "Italic"), "Ctrl+I", "*", "*"),
            (tr("Barrato", "Strikethrough"), "Ctrl+Shift+X", "~~", "~~"),
            (tr("Codice", "Code"), "Ctrl+Shift+C", "`", "`"),
        ] {
            if item(ui, label, sc, true) {
                cmd = Some(MenuCmd::Wrap(open, close));
            }
        }
        if item(ui, tr("Link", "Link"), "Ctrl+K", true) {
            cmd = Some(MenuCmd::Link);
        }
    });
    ui.menu_button(tr("Paragrafo", "Paragraph"), |ui| {
        ui.set_min_width(210.0);
        for (label, sc, block) in [
            (tr("Titolo 1", "Heading 1"), "Ctrl+1", Block::Heading(1)),
            (tr("Titolo 2", "Heading 2"), "Ctrl+2", Block::Heading(2)),
            (tr("Titolo 3", "Heading 3"), "Ctrl+3", Block::Heading(3)),
            (tr("Testo normale", "Normal text"), "Ctrl+0", Block::Heading(0)),
        ] {
            if item(ui, label, sc, true) {
                cmd = Some(MenuCmd::Block(block));
            }
        }
        ui.separator();
        for (label, sc, block) in [
            (tr("Checklist", "Checklist"), "Ctrl+L", Block::Task),
            (tr("Elenco puntato", "Bulleted list"), "", Block::Bullet),
            (tr("Elenco numerato", "Numbered list"), "", Block::Numbered),
            (tr("Citazione", "Quote"), "", Block::Quote),
        ] {
            if item(ui, label, sc, true) {
                cmd = Some(MenuCmd::Block(block));
            }
        }
    });
    ui.menu_button(tr("Inserisci", "Insert"), |ui| {
        ui.set_min_width(210.0);
        if item(ui, tr("Immagine dal PC…", "Image from PC…"), "", true) {
            cmd = Some(MenuCmd::InsertImage);
        }
        if item(ui, tr("Link a una nota", "Link to a note"), "[[", true) {
            cmd = Some(MenuCmd::InsertNoteLink);
        }
        if item(ui, tr("Separatore", "Divider"), "---", true) {
            cmd = Some(MenuCmd::InsertRule);
        }
    });
    ui.separator();
    let pin = if cx.pinned { tr("Sblocca la nota", "Unpin note") } else { tr("Fissa la nota in alto", "Pin note to top") };
    if item(ui, pin, "Ctrl+Shift+P", cx.has_file && cx.can_pin) {
        cmd = Some(MenuCmd::File(FileCmd::Pin));
    }
    if item(ui, tr("Mostra in Esplora file", "Show in File Explorer"), "", cx.has_file) {
        cmd = Some(MenuCmd::File(FileCmd::Reveal));
    }
    if item(ui, tr("Copia il percorso del file", "Copy file path"), "", cx.has_file) {
        cmd = Some(MenuCmd::File(FileCmd::CopyPath));
    }
    ui.separator();
    let danger = theme::palette(ui.ctx()).danger;
    let delete = egui::Button::new(egui::RichText::new(tr("Sposta nel Cestino", "Move to Recycle Bin")).color(danger));
    if ui.add_enabled(cx.has_file, delete).clicked() {
        cmd = Some(MenuCmd::File(FileCmd::Delete));
    }
    if cmd.is_some() {
        ui.close();
    }
    cmd
}

/// Codifica un'immagine RGBA in PNG e la salva negli allegati.
fn save_png(cx: &EditorCtx<'_>, w: usize, h: usize, rgba: Vec<u8>) -> Result<PathBuf, String> {
    let img = image::RgbaImage::from_raw(w as u32, h as u32, rgba).ok_or(tr("immagine non valida", "invalid image"))?;
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    vault::save_attachment(cx.vault_root, &png, "png")
        .map_err(|e| format!("{}: {e}", tr("Impossibile salvare l'immagine", "Could not save the image")))
}

/// Copia un file immagine negli allegati.
fn import_image(cx: &EditorCtx<'_>, file: &Path) -> Result<PathBuf, String> {
    let ext = file.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_else(|| "png".into());
    let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
    vault::save_attachment(cx.vault_root, &bytes, &ext)
        .map_err(|e| format!("{}: {e}", tr("Impossibile salvare l'immagine", "Could not save the image")))
}

// ---------------------------------------------------------------------------
// Aiuti per cursore e testo

/// Selezione attuale in byte (inizio ≤ fine).
fn cursor_bytes(ui: &egui::Ui, text: &str) -> Option<(usize, usize)> {
    let state = egui::TextEdit::load_state(ui.ctx(), editor_id())?;
    let r = state.cursor.char_range()?;
    let (x, y) = (r.primary.index.0, r.secondary.index.0);
    Some((char_to_byte(text, x.min(y)), char_to_byte(text, x.max(y))))
}

fn set_cursor(ui: &egui::Ui, text: &str, a: usize, b: usize) {
    let ctx = ui.ctx();
    let mut state = egui::TextEdit::load_state(ctx, editor_id()).unwrap_or_default();
    let (ca, cb) = (byte_to_char(text, a), byte_to_char(text, b));
    state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(ca), CCursor::new(cb))));
    egui::TextEdit::store_state(ctx, editor_id(), state);
}

fn wrap_selection(ui: &egui::Ui, text: &mut String, a: usize, b: usize, open: &str, close: &str) -> bool {
    // Se la selezione è già avvolta, la sintassi viene tolta.
    if a >= open.len() && text[..a].ends_with(open) && text[b..].starts_with(close) {
        text.replace_range(b..b + close.len(), "");
        text.replace_range(a - open.len()..a, "");
        set_cursor(ui, text, a - open.len(), b - open.len());
    } else {
        text.insert_str(b, close);
        text.insert_str(a, open);
        set_cursor(ui, text, a + open.len(), b + open.len());
    }
    true
}

/// Applica un formato di paragrafo alle righe selezionate e aggiorna la selezione.
fn set_block(ui: &egui::Ui, text: &mut String, a: usize, b: usize, block: Block) {
    let (new_text, na, nb) = apply_block(text, a, b, block);
    *text = new_text;
    set_cursor(ui, text, na, nb);
}

/// Sostituisce i primi `old_len` byte della riga che inizia in `start` con `new`.
fn replace_prefix(ui: &egui::Ui, text: &mut String, start: usize, old_len: usize, new: &str, cursor: usize) -> bool {
    text.replace_range(start..start + old_len, new);
    let pos = if cursor >= start + old_len { cursor - old_len + new.len() } else { start + new.len() };
    let pos = pos.min(text.len());
    set_cursor(ui, text, pos, pos);
    true
}

fn target_at(lines: &[Line], byte: usize) -> Option<Target> {
    lines.iter().find(|l| l.range.start <= byte && byte < l.range.end)?.spans.iter().find(|s| s.range.contains(&byte))?.target.clone()
}

/// Costruisce l'impaginazione del testo con gli stili dell'anteprima live.
fn layout_job(
    text: &str,
    lines: &[Line],
    cursor_line: Option<usize>,
    base: f32,
    spacing: f32,
    v: &egui::Visuals,
    images: &HashMap<String, f32>,
) -> LayoutJob {
    let mut job = LayoutJob { text: text.to_owned(), ..Default::default() };
    let text_color = v.text_color();
    let weak = v.weak_text_color();
    let accent = v.hyperlink_color;

    let push = |job: &mut LayoutJob, range: std::ops::Range<usize>, format: TextFormat| {
        if !range.is_empty() {
            job.sections.push(LayoutSection { leading_space: 0.0, byte_range: ByteIndex(range.start)..ByteIndex(range.end), format });
        }
    };

    for (n, line) in lines.iter().enumerate() {
        let active = cursor_line == Some(n);
        let size = base * scale(line.kind);
        let heading = matches!(line.kind, LineKind::Title | LineKind::Heading(_));
        let mono = matches!(line.kind, LineKind::Code | LineKind::CodeFence);
        let mut line_height = Some(size * if heading { 1.3 } else { spacing });
        if line.kind == LineKind::Image
            && !active
            && let Some(h) = line.src.as_ref().and_then(|s| images.get(s))
        {
            line_height = Some(h + 14.0);
        }
        let muted_line = matches!(line.kind, LineKind::Task { checked: true });

        let fmt_for = |f: Fmt| -> TextFormat {
            let family = if mono || f.code { FontFamily::Monospace } else { theme::editor_family(f.bold || heading, f.italic) };
            let font_size = if f.code && !mono { size * 0.88 } else { size };
            let mut color = if f.link || f.tag {
                accent
            } else if muted_line || line.kind == LineKind::Quote {
                weak
            } else {
                text_color
            };
            if heading && !f.link && !f.tag {
                color = v.strong_text_color();
            }
            let mut tf = TextFormat { font_id: FontId::new(font_size, family), color, line_height, ..Default::default() };
            if f.code || mono {
                tf.background = v.code_bg_color;
            }
            if f.strike {
                tf.strikethrough = Stroke::new(1.0, weak);
            }
            if f.link {
                tf.underline = Stroke::new(1.0, accent.gamma_multiply(0.45));
            }
            if f.tag {
                tf.background = accent.gamma_multiply(0.14);
            }
            if f.marker {
                if active {
                    tf.color = weak.gamma_multiply(0.85);
                    tf.background = Color32::TRANSPARENT;
                    tf.underline = Stroke::NONE;
                    tf.strikethrough = Stroke::NONE;
                } else {
                    // Nascosto: carattere minuscolo e trasparente.
                    tf.font_id.size = HIDDEN_SIZE;
                    tf.color = Color32::TRANSPARENT;
                    tf.background = Color32::TRANSPARENT;
                    tf.underline = Stroke::NONE;
                    tf.strikethrough = Stroke::NONE;
                }
            }
            tf
        };

        // Prefisso del blocco.
        let normal = fmt_for(Fmt::default());
        let prefix_fmt = match line.kind {
            // Puntini e checkbox sono disegnati sopra: il prefisso è nascosto e
            // il testo parte da un rientro fisso (uguale per "[ ]" e "[x]").
            LineKind::Bullet | LineKind::Task { .. } => {
                TextFormat { font_id: FontId::new(HIDDEN_SIZE, FontFamily::Proportional), color: Color32::TRANSPARENT, ..normal.clone() }
            }
            LineKind::Quote => TextFormat { color: Color32::TRANSPARENT, ..normal.clone() },
            LineKind::Rule if !active => TextFormat { color: Color32::TRANSPARENT, ..normal.clone() },
            LineKind::Image if !active => {
                TextFormat { font_id: FontId::new(HIDDEN_SIZE, FontFamily::Proportional), color: Color32::TRANSPARENT, ..normal.clone() }
            }
            LineKind::Image => TextFormat { color: weak, ..normal.clone() },
            LineKind::Ordered => {
                TextFormat { color: weak, font_id: FontId::new(size, theme::editor_family(false, false)), ..normal.clone() }
            }
            LineKind::CodeFence | LineKind::Rule => TextFormat { color: weak, ..normal.clone() },
            _ => fmt_for(Fmt { marker: true, ..Fmt::default() }),
        };
        push(&mut job, line.prefix.clone(), prefix_fmt);
        let first_section = job.sections.len();
        for span in &line.spans {
            push(&mut job, span.range.clone(), fmt_for(span.fmt));
        }
        if line.range.end < text.len() {
            push(&mut job, line.range.end..line.range.end + 1, normal);
        }
        let gutter = match line.kind {
            LineKind::Task { .. } => Some(size * 1.62),
            LineKind::Bullet => Some(size * 1.2),
            _ => None,
        };
        if let (Some(g), Some(section)) = (gutter, job.sections.get_mut(first_section)) {
            section.leading_space = list_indent(&text[line.range.clone()], size) + g;
        }
    }
    job
}

/// Rientro degli elenchi annidati: due spazi (o un tab) per livello.
fn list_indent(line: &str, size: f32) -> f32 {
    let units: usize = line.chars().take_while(|c| c.is_whitespace()).map(|c| if c == '\t' { 2 } else { 1 }).sum();
    (units / 2) as f32 * size * 1.4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_apply_to_every_selected_line() {
        let t = "Titolo\nuno\ndue\n\ntre\nfuori";
        let (a, b) = (t.find("uno").unwrap(), t.find("tre").unwrap() + 3);
        let (n, ..) = apply_block(t, a, b, Block::Numbered);
        assert_eq!(n, "Titolo\n1. uno\n2. due\n\n3. tre\nfuori");
        // Già tutte checklist: il comando le toglie.
        let (c, ..) = apply_block(t, a, b, Block::Task);
        assert_eq!(c, "Titolo\n- [ ] uno\n- [ ] due\n\n- [ ] tre\nfuori");
        let (a2, b2) = (c.find("- [ ] uno").unwrap(), c.find("tre").unwrap() + 3);
        let (back, ..) = apply_block(&c, a2, b2, Block::Task);
        assert_eq!(back, t);
        // Da elenco numerato a puntato, e titoli.
        let (bul, ..) = apply_block(&n, n.find("1.").unwrap(), n.find("tre").unwrap(), Block::Bullet);
        assert_eq!(bul, "Titolo\n- uno\n- due\n\n- tre\nfuori");
        let (h, ..) = apply_block(t, a, a, Block::Heading(2));
        assert_eq!(h, "Titolo\n## uno\ndue\n\ntre\nfuori");
        // Una selezione che finisce all'inizio della riga dopo non la include.
        let sel_end = t.find("due").unwrap();
        let (one, ..) = apply_block(t, a, sel_end, Block::Bullet);
        assert_eq!(one, "Titolo\n- uno\ndue\n\ntre\nfuori");
    }

    #[test]
    fn images_fit_the_column() {
        assert_eq!(image_display_size(Vec2::new(2000.0, 1000.0), 700.0), Vec2::new(700.0, 350.0));
        assert_eq!(image_display_size(Vec2::new(200.0, 100.0), 700.0), Vec2::new(200.0, 100.0));
        assert!(image_display_size(Vec2::new(500.0, 3000.0), 700.0).y <= 460.0);
    }

    #[test]
    fn fuzzy_finds_titles() {
        let items = vec!["Lista della spesa".to_owned(), "Progetto X".to_owned(), "Ricette".to_owned()];
        assert_eq!(vault::fuzzy_rank("spesa", &items, 5)[0], "Lista della spesa");
        assert_eq!(vault::fuzzy_rank("", &items, 2).len(), 2);
    }

    #[test]
    fn parse_cache_reuses_unchanged_text() {
        let mut cache = (0, Arc::new(Vec::new()));
        let a = parse_cached(&mut cache, "Titolo\n- voce");
        let b = parse_cached(&mut cache, "Titolo\n- voce");
        assert!(Arc::ptr_eq(&a, &b));
        let c = parse_cached(&mut cache, "Titolo\n- altra");
        assert!(!Arc::ptr_eq(&a, &c));
    }
}
