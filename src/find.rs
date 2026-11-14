//! Trova e sostituisci nella nota aperta (Ctrl+F / Ctrl+H).
//!
//! La ricerca ignora maiuscole e minuscole. Le corrispondenze sono intervalli
//! in byte del testo della nota; l'editor le evidenzia e scorre fino a quella attiva.

use crate::i18n::tr;
use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets;
use eframe::egui::{self, Color32, FontId, Id, Key, Modifiers, Pos2, Rect, Stroke, StrokeKind, Vec2, text::CCursor, text::CCursorRange};
use std::ops::Range;

/// Oltre questo numero di corrispondenze si smette di contare (note enormi, query di una lettera).
const MAX_MATCHES: usize = 10_000;

#[derive(Default)]
pub struct Find {
    pub open: bool,
    /// Riga "Sostituisci con" visibile.
    pub replace: bool,
    pub query: String,
    pub replacement: String,
    /// Corrispondenze nel testo attuale (intervalli in byte).
    pub matches: Vec<Range<usize>>,
    /// Indice della corrispondenza attiva in `matches`.
    pub current: usize,
    /// L'editor deve scorrere fino alla corrispondenza attiva.
    pub scroll: bool,
    /// Il campo di ricerca deve prendere il focus (con il testo selezionato).
    focus: bool,
    /// Dove era il cursore della nota all'apertura (in byte): la ricerca parte da lì.
    from: usize,
}

/// Cosa è successo nella barra in questo fotogramma.
pub enum BarOutcome {
    None,
    /// Il testo della nota è cambiato (sostituzione).
    Changed,
    /// Barra chiusa: l'editor riprende il focus, con la corrispondenza attiva selezionata.
    Closed(Option<Range<usize>>),
}

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Tutte le occorrenze di `query` in `text`, senza distinguere maiuscole e minuscole.
pub fn find_all(text: &str, query: &str) -> Vec<Range<usize>> {
    let q: Vec<char> = query.chars().map(fold).collect();
    if q.is_empty() {
        return Vec::new();
    }
    let t: Vec<(usize, char)> = text.char_indices().map(|(i, c)| (i, fold(c))).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + q.len() <= t.len() && out.len() < MAX_MATCHES {
        if t[i..i + q.len()].iter().zip(&q).all(|(a, b)| a.1 == *b) {
            let end = t.get(i + q.len()).map_or(text.len(), |x| x.0);
            out.push(t[i].0..end);
            i += q.len();
        } else {
            i += 1;
        }
    }
    out
}

/// Sostituisce tutte le corrispondenze; restituisce quante erano.
pub fn replace_all(text: &mut String, query: &str, replacement: &str) -> usize {
    let matches = find_all(text, query);
    for r in matches.iter().rev() {
        text.replace_range(r.clone(), replacement);
    }
    matches.len()
}

impl Find {
    /// Apre la barra. Se c'è del testo selezionato su una sola riga, diventa la ricerca.
    pub fn show(&mut self, replace: bool, selected: Option<&str>, from: usize) {
        self.from = from;
        if let Some(s) = selected.filter(|s| !s.is_empty() && !s.contains('\n')) {
            self.query = s.to_owned();
        }
        self.open = true;
        self.replace = replace;
        self.focus = true;
        self.scroll = true;
    }

    /// Ricalcola le corrispondenze; se la ricerca è cambiata parte dalla prima dopo `from`.
    pub fn refresh(&mut self, text: &str, from: Option<usize>) {
        self.matches = find_all(text, &self.query);
        if let Some(from) = from {
            self.current = self.matches.iter().position(|r| r.start >= from).unwrap_or(0);
        }
        if self.current >= self.matches.len() {
            self.current = 0;
        }
    }

    fn step(&mut self, delta: isize) {
        let n = self.matches.len();
        if n > 0 {
            self.current = (self.current as isize + delta).rem_euclid(n as isize) as usize;
            self.scroll = true;
        }
    }

    pub fn active(&self) -> Option<Range<usize>> {
        self.matches.get(self.current).cloned()
    }

    /// Barra in alto a destra dell'editor; `right_top` è il suo angolo in alto a destra.
    pub fn bar(&mut self, ctx: &egui::Context, right_top: Pos2, text: &mut String) -> BarOutcome {
        if !self.open {
            return BarOutcome::None;
        }
        self.refresh(text, None);
        let mut outcome = BarOutcome::None;
        let mut close = false;
        let p = theme::palette(ctx);

        // F3 / Maiusc+F3 funzionano anche mentre si scrive nella nota.
        if ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::F3)) {
            self.step(-1);
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F3)) {
            self.step(1);
        }

        let width = 420.0;
        let query_id = Id::unique("yonote-find-query");
        let replace_id = Id::unique("yonote-find-replace");
        egui::Area::new(Id::unique("yonote-find")).fixed_pos(right_top - Vec2::new(width, 0.0)).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(8).show(ui, |ui| {
                ui.set_width(width - 16.0);
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 6.0);
                let tail = 50.0 + 3.0 * 28.0 + 3.0 * 4.0;

                ui.horizontal(|ui| {
                    let (icon, tip) = if self.replace {
                        (Icon::ChevronDown, tr("Nascondi Sostituisci", "Hide Replace"))
                    } else {
                        (Icon::ChevronRight, tr("Mostra Sostituisci (Ctrl+H)", "Show Replace (Ctrl+H)"))
                    };
                    if icons::button_sized(ui, icon, tip, 24.0, 14.0).clicked() {
                        self.replace = !self.replace;
                    }
                    let field_w = ui.available_width() - tail;
                    let before = self.query.clone();
                    let q = ui
                        .allocate_ui_with_layout(Vec2::new(field_w, 32.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            widgets::search_field(ui, &mut self.query, tr("Trova nella nota", "Find in note"), query_id)
                        })
                        .inner;
                    if self.focus {
                        select_all(ui.ctx(), query_id, &self.query);
                        q.request_focus();
                        self.focus = false;
                    }
                    if self.query != before {
                        self.refresh(text, Some(self.from));
                        self.scroll = true;
                    }
                    let (enter, shift, esc) = ui.input(|i| (i.key_pressed(Key::Enter), i.modifiers.shift, i.key_pressed(Key::Escape)));
                    if q.lost_focus() && enter {
                        self.step(if shift { -1 } else { 1 });
                        q.request_focus();
                    }
                    close |= q.lost_focus() && esc;

                    // Contatore "3/12" (rosso se la ricerca non trova nulla).
                    let (count, color) = match (self.query.is_empty(), self.matches.len()) {
                        (true, _) => (String::new(), p.text_muted),
                        (false, 0) => ("0/0".to_owned(), p.danger),
                        (false, n) if n >= MAX_MATCHES => (format!("{}/{n}+", self.current + 1), p.text_muted),
                        (false, n) => (format!("{}/{n}", self.current + 1), p.text_muted),
                    };
                    let (r, _) = ui.allocate_exact_size(Vec2::new(50.0, 28.0), egui::Sense::hover());
                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, count, FontId::proportional(12.5), color);

                    if icons::button_sized(ui, Icon::ChevronUp, tr("Precedente (Maiusc+Invio)", "Previous (Shift+Enter)"), 28.0, 16.0)
                        .clicked()
                    {
                        self.step(-1);
                    }
                    if icons::button_sized(ui, Icon::ChevronDown, tr("Successivo (Invio)", "Next (Enter)"), 28.0, 16.0).clicked() {
                        self.step(1);
                    }
                    if icons::button_sized(ui, Icon::Close, tr("Chiudi (Esc)", "Close (Esc)"), 28.0, 15.0).clicked() {
                        close = true;
                    }
                });

                if self.replace {
                    ui.horizontal(|ui| {
                        ui.add_space(24.0 + 4.0);
                        let field_w = ui.available_width() - tail;
                        let r = ui
                            .allocate_ui_with_layout(Vec2::new(field_w, 32.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                text_field(ui, &mut self.replacement, tr("Sostituisci con", "Replace with"), replace_id)
                            })
                            .inner;
                        let (enter, esc) = ui.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
                        let mut one = r.lost_focus() && enter;
                        if one {
                            r.request_focus();
                        }
                        close |= r.lost_focus() && esc;
                        let has = !self.matches.is_empty();
                        let one_btn = ui
                            .add_enabled(has, egui::Button::new(tr("Sostituisci", "Replace")).min_size(Vec2::new(0.0, 28.0)))
                            .on_hover_text(tr("Sostituisci questa (Invio)", "Replace this one (Enter)"));
                        one |= one_btn.clicked();
                        let all = ui
                            .add_enabled(has, egui::Button::new(tr("Tutte", "All")).min_size(Vec2::new(0.0, 28.0)))
                            .on_hover_text(tr("Sostituisci tutte (Ctrl+Alt+Invio)", "Replace all (Ctrl+Alt+Enter)"))
                            .clicked()
                            || ui.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::ALT, Key::Enter));
                        if all && has {
                            replace_all(text, &self.query, &self.replacement);
                            self.refresh(text, None);
                            outcome = BarOutcome::Changed;
                        } else if one && let Some(m) = self.active() {
                            text.replace_range(m.clone(), &self.replacement);
                            // La prossima corrispondenza è quella dopo il testo appena inserito.
                            self.refresh(text, Some(m.start + self.replacement.len()));
                            self.scroll = true;
                            outcome = BarOutcome::Changed;
                        }
                    });
                }
            });
        });

        if close {
            self.open = false;
            return BarOutcome::Closed(self.active());
        }
        outcome
    }

    /// Rettangoli evidenziati (sotto il testo) per le corrispondenze visibili, e quello della attiva.
    /// `rect_of(a, b)` dà i rettangoli dei caratteri da `a` a `b` (indici in caratteri).
    pub fn highlights(&self, text: &str, clip: Rect, dark: bool, rect_of: impl Fn(usize, usize) -> Vec<Rect>) -> (Vec<egui::Shape>, Option<Rect>) {
        let (other, active, stroke) = if dark {
            (Color32::from_rgba_unmultiplied(255, 196, 60, 55), Color32::from_rgba_unmultiplied(255, 160, 40, 140), Color32::from_rgb(255, 170, 60))
        } else {
            (Color32::from_rgba_unmultiplied(255, 200, 40, 110), Color32::from_rgba_unmultiplied(255, 150, 0, 150), Color32::from_rgb(230, 120, 0))
        };
        let mut shapes = Vec::new();
        let mut active_rect = None;
        let (mut chars, mut last) = (0, 0);
        for (i, m) in self.matches.iter().enumerate() {
            chars += text[last..m.start].chars().count();
            last = m.start;
            let len = text[m.clone()].chars().count();
            let is_active = i == self.current;
            let rects = rect_of(chars, chars + len);
            let Some(first) = rects.first().copied() else { continue };
            if is_active {
                active_rect = Some(rects.iter().fold(first, |a, r| a.union(*r)));
            }
            if first.min.y > clip.max.y && !is_active && active_rect.is_some() {
                break;
            }
            if first.max.y < clip.min.y || first.min.y > clip.max.y {
                continue;
            }
            for r in rects {
                // Più bassa della riga, così le corrispondenze su righe vicine restano separate.
                let r = r.expand2(Vec2::new(1.0, -r.height() * 0.13));
                shapes.push(egui::Shape::rect_filled(r, 3.0, if is_active { active } else { other }));
                if is_active {
                    shapes.push(egui::Shape::rect_stroke(r, 3.0, Stroke::new(1.0, stroke), StrokeKind::Outside));
                }
            }
        }
        (shapes, active_rect)
    }
}

/// Seleziona tutto il testo di un campo (per riscrivere subito la ricerca).
fn select_all(ctx: &egui::Context, id: Id, text: &str) {
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(text.chars().count()))));
    egui::TextEdit::store_state(ctx, id, state);
}

/// Campo di testo con lo stesso aspetto del campo di ricerca, senza icona.
fn text_field(ui: &mut egui::Ui, text: &mut String, hint: &str, id: Id) -> egui::Response {
    let p = theme::palette(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 8.0, ui.visuals().extreme_bg_color);
    if ui.memory(|m| m.has_focus(id)) {
        ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.5, p.accent_text), StrokeKind::Inside);
    }
    let inner = rect.shrink2(Vec2::new(11.0, 0.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
        ui.centered_and_justified(|ui| {
            ui.add(
                egui::TextEdit::singleline(text)
                    .id(id)
                    .frame(egui::Frame::NONE)
                    .hint_text(hint)
                    .align(egui::Align2::LEFT_CENTER)
                    .font(FontId::proportional(14.0)),
            )
        })
        .inner
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_ignoring_case_and_replaces() {
        let text = "Caffè e caffè, CAFFÈ.";
        let m = find_all(text, "caffè");
        assert_eq!(m.len(), 3);
        assert_eq!(&text[m[2].clone()], "CAFFÈ");
        let mut t = text.to_owned();
        assert_eq!(replace_all(&mut t, "CAFFÈ", "tè"), 3);
        assert_eq!(t, "tè e tè, tè.");
        assert!(find_all(text, "").is_empty());
        assert_eq!(find_all("aaaa", "aa").len(), 2);
    }

    #[test]
    fn starts_after_the_cursor_and_wraps() {
        let mut f = Find { query: "x".into(), ..Default::default() };
        f.refresh("x . x . x", Some(3));
        assert_eq!(f.current, 1);
        f.step(1);
        f.step(1);
        assert_eq!(f.current, 0);
        f.step(-1);
        assert_eq!(f.current, 2);
    }
}
