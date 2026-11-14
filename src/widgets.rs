//! Componenti dell'interfaccia: righe della barra laterale e dell'elenco,
//! campo di ricerca, selettori segmentati, campioni di colore, interruttori.

use crate::icons::{self, Icon};
use crate::theme::{self, Palette};
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Vec2};

pub fn pal(ui: &egui::Ui) -> Palette {
    theme::palette(ui.ctx())
}

/// Nome e ruolo per i lettori di schermo (i componenti disegnati a mano non li hanno da soli).
pub fn a11y(resp: &Response, role: egui::Role, label: &str, selected: Option<bool>) {
    let enabled = resp.enabled();
    resp.widget_info(|| match selected {
        Some(s) => egui::WidgetInfo::selected(role, enabled, s, label),
        None => egui::WidgetInfo::labeled(role, enabled, label),
    });
}

/// Contorno di focus da tastiera, uguale per tutti i componenti.
fn focus_ring(ui: &egui::Ui, resp: &Response, rect: Rect, radius: f32) {
    if resp.has_focus() {
        ui.painter().rect_stroke(rect, radius, Stroke::new(1.5, pal(ui).accent_text), StrokeKind::Inside);
    }
}

/// Etichetta di un gruppo nella barra laterale ("Cartelle", "Tag").
pub fn group_label(ui: &mut egui::Ui, text: &str) -> Rect {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
    let p = pal(ui);
    ui.painter().text(
        Pos2::new(rect.min.x + 10.0, rect.max.y - 6.0),
        Align2::LEFT_BOTTOM,
        text,
        FontId::new(12.0, theme::family(true, false)),
        p.text_muted,
    );
    rect
}

/// Riga della barra laterale con icona, testo e conteggio.
pub fn nav_row(ui: &mut egui::Ui, icon: Icon, label: &str, count: Option<usize>, selected: bool, depth: usize) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
    a11y(&resp, egui::Role::Button, label, Some(selected));
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let p = pal(ui);
    if selected {
        ui.painter().rect_filled(rect, 7.0, p.hover.gamma_multiply(2.6));
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 7.0, p.hover);
    }
    focus_ring(ui, &resp, rect, 7.0);
    let x = rect.min.x + 10.0 + depth as f32 * 16.0;
    let icon_color = if selected { p.accent_text } else { p.text_muted };
    icons::paint(ui, icon, Rect::from_center_size(Pos2::new(x + 8.0, rect.center().y), Vec2::splat(17.0)), icon_color);
    let text_rect = Rect::from_min_max(Pos2::new(x + 24.0, rect.min.y), Pos2::new(rect.max.x - 34.0, rect.max.y));
    ui.painter().with_clip_rect(text_rect).text(
        Pos2::new(text_rect.min.x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::new(14.0, theme::family(selected, false)),
        p.text,
    );
    if let Some(c) = count {
        ui.painter().text(
            Pos2::new(rect.max.x - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            c.to_string(),
            FontId::proportional(12.5),
            p.text_muted,
        );
    }
    resp
}

/// Dati di una riga dell'elenco note.
pub struct NoteRow<'a> {
    pub title: &'a str,
    pub date: &'a str,
    pub preview: &'a str,
    pub thumb: Option<&'a std::path::Path>,
    pub pinned: bool,
    /// La nota aperta.
    pub selected: bool,
    /// Selezionata con Ctrl/Maiusc.
    pub marked: bool,
}

/// Riga dell'elenco note: titolo, data + anteprima, miniatura opzionale.
pub fn note_row(ui: &mut egui::Ui, row: &NoteRow<'_>) -> Response {
    let NoteRow { title, date, preview, thumb, pinned, selected, marked } = *row;
    let height = 64.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    a11y(&resp, egui::Role::ListItem, &format!("{title}, {date}"), Some(selected || marked));
    // Le righe fuori dallo schermo non si disegnano (e le miniature non si caricano).
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let p = pal(ui);
    if selected {
        ui.painter().rect_filled(rect, 10.0, p.accent_fill);
    } else if marked {
        // Selezionata con Ctrl/Maiusc ma non aperta: tinta più leggera e bordo d'accento.
        ui.painter().rect_filled(rect, 10.0, p.accent_fill.gamma_multiply(0.55));
        ui.painter().rect_stroke(rect, 10.0, Stroke::new(1.0, p.accent_text.gamma_multiply(0.6)), StrokeKind::Inside);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 10.0, p.hover);
    }
    focus_ring(ui, &resp, rect, 10.0);
    let mut inner = rect.shrink2(Vec2::new(14.0, 11.0));
    if let Some(path) = thumb {
        let side = height - 22.0;
        let r = Rect::from_min_size(Pos2::new(inner.max.x - side, inner.min.y), Vec2::splat(side));
        let uri = crate::vault::file_uri(path);
        egui::Image::new(uri).corner_radius(6).paint_at(ui, r);
        inner.max.x -= side + 10.0;
    }
    let clip = ui.painter().with_clip_rect(inner);
    let mut tx = inner.min.x;
    if pinned {
        icons::paint(ui, Icon::Pin, Rect::from_min_size(Pos2::new(tx - 1.0, inner.min.y + 1.0), Vec2::splat(15.0)), p.accent_text);
        tx += 17.0;
    }
    clip.text(Pos2::new(tx, inner.min.y), Align2::LEFT_TOP, title, FontId::new(14.5, theme::family(true, false)), p.text);
    let date_galley = ui.painter().layout_no_wrap(date.to_owned(), FontId::new(12.5, theme::family(true, false)), p.text_muted);
    let dw = date_galley.size().x;
    clip.galley(Pos2::new(inner.min.x, inner.max.y - date_galley.size().y), date_galley, p.text_muted);
    clip.text(Pos2::new(inner.min.x + dw + 8.0, inner.max.y), Align2::LEFT_BOTTOM, preview, FontId::proportional(12.5), p.text_muted);
    resp
}

/// Linea sottile tra le righe dell'elenco (come in Notes), rientrata.
pub fn row_divider(ui: &mut egui::Ui) {
    let p = pal(ui);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(rect.min.x + 14.0..=rect.max.x - 14.0, rect.center().y, Stroke::new(1.0, p.border));
}

/// Campo di ricerca con icona e pulsante per svuotarlo.
pub fn search_field(ui: &mut egui::Ui, text: &mut String, hint: &str, id: Id) -> Response {
    let p = pal(ui);
    let height = 32.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let focused = ui.memory(|m| m.has_focus(id));
    ui.painter().rect_filled(rect, 8.0, ui.visuals().extreme_bg_color);
    if focused {
        ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.5, p.accent_text), StrokeKind::Inside);
    }
    icons::paint(ui, Icon::Search, Rect::from_center_size(Pos2::new(rect.min.x + 17.0, rect.center().y), Vec2::splat(16.0)), p.text_muted);
    let edit_rect = Rect::from_min_max(Pos2::new(rect.min.x + 32.0, rect.min.y), Pos2::new(rect.max.x - 30.0, rect.max.y));
    let resp = ui
        .scope_builder(egui::UiBuilder::new().max_rect(edit_rect), |ui| {
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
        .inner;
    if !text.is_empty() {
        let r = Rect::from_center_size(Pos2::new(rect.max.x - 16.0, rect.center().y), Vec2::splat(22.0));
        let clear = ui.interact(r, id.with("clear"), Sense::click());
        a11y(&clear, egui::Role::Button, crate::i18n::tr("Svuota la ricerca", "Clear search"), None);
        if clear.hovered() {
            ui.painter().rect_filled(r, 5.0, p.hover);
        }
        icons::paint(ui, Icon::Close, r.shrink(5.0), p.text_muted);
        if clear.clicked() {
            text.clear();
            resp.request_focus();
        }
    }
    resp
}

/// Selettore a segmenti (es. Sistema | Chiaro | Scuro).
pub fn segmented<T: PartialEq + Copy>(ui: &mut egui::Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let p = pal(ui);
    let font = FontId::proportional(13.5);
    let widths: Vec<f32> =
        options.iter().map(|(_, l)| ui.painter().layout_no_wrap((*l).to_owned(), font.clone(), p.text).size().x + 28.0).collect();
    let total: f32 = widths.iter().sum::<f32>() + 6.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total, 32.0), Sense::hover());
    ui.painter().rect_filled(rect, 9.0, ui.visuals().extreme_bg_color);
    let mut x = rect.min.x + 3.0;
    let mut changed = false;
    for ((opt, label), w) in options.iter().zip(widths) {
        let r = Rect::from_min_size(Pos2::new(x, rect.min.y + 3.0), Vec2::new(w, 26.0));
        let resp = ui.interact(r, ui.scope_id().with(("seg", label)), Sense::click());
        let selected = *value == *opt;
        a11y(&resp, egui::Role::RadioButton, label, Some(selected));
        focus_ring(ui, &resp, r, 7.0);
        if selected {
            ui.painter().rect_filled(r, 7.0, if p.dark { Color32::from_white_alpha(28) } else { Color32::WHITE });
            ui.painter().rect_stroke(r, 7.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 7.0, p.hover);
        }
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            *label,
            FontId::new(13.5, theme::family(selected, false)),
            if selected { p.text } else { p.text_muted },
        );
        if resp.clicked() && !selected {
            *value = *opt;
            changed = true;
        }
        x += w;
    }
    changed
}

/// Cerchio di colore per scegliere l'accento.
pub fn swatch(ui: &mut egui::Ui, color: Color32, selected: bool, tooltip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
    a11y(&resp, egui::Role::RadioButton, tooltip, Some(selected));
    focus_ring(ui, &resp, rect, 15.0);
    let p = pal(ui);
    let c = rect.center();
    let mono = color == Color32::from_rgb(theme::MONO[0], theme::MONO[1], theme::MONO[2]);
    let r = if selected {
        9.5
    } else if resp.hovered() {
        11.5
    } else {
        11.0
    };
    if selected {
        ui.painter().circle_stroke(c, 13.5, Stroke::new(2.0, if mono { p.text } else { color }));
    }
    if mono {
        // Metà nera e metà bianca: l'accento che segue il tema.
        ui.painter().circle_filled(c, r, Color32::WHITE);
        let half: Vec<Pos2> = (0..=24)
            .map(|i| std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 24.0)
            .map(|a| c + Vec2::angled(a) * r)
            .collect();
        ui.painter().add(egui::Shape::convex_polygon(half, Color32::from_gray(12), Stroke::NONE));
        ui.painter().circle_stroke(c, r, Stroke::new(1.0, p.border.gamma_multiply(2.0)));
    } else {
        ui.painter().circle_filled(c, r, color);
        if !selected {
            ui.painter().circle_stroke(c, r, Stroke::new(1.0, p.border));
        }
    }
    resp.on_hover_text(tooltip)
}

/// Pulsante piccolo e arrotondato con icona e testo (es. "Aggiorna a 0.3.0").
pub fn pill(ui: &mut egui::Ui, icon: Icon, label: &str) -> Response {
    let p = pal(ui);
    let font = FontId::new(12.5, theme::family(true, false));
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, p.text);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 40.0, 28.0), Sense::click());
    a11y(&resp, egui::Role::Button, label, None);
    let fill = if resp.hovered() { p.accent_fill.gamma_multiply(1.6) } else { p.accent_fill };
    ui.painter().rect_filled(rect, 14.0, fill);
    ui.painter().rect_stroke(rect, 14.0, Stroke::new(1.0, p.accent_text.gamma_multiply(0.35)), StrokeKind::Inside);
    focus_ring(ui, &resp, rect, 14.0);
    icons::paint(ui, icon, Rect::from_center_size(Pos2::new(rect.min.x + 17.0, rect.center().y), Vec2::splat(15.0)), p.accent_text);
    ui.painter().galley(Pos2::new(rect.min.x + 29.0, rect.center().y - galley.size().y / 2.0), galley, p.text);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Interruttore on/off in stile Windows 11.
pub fn toggle(ui: &mut egui::Ui, on: &mut bool, label: &str) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(40.0, 20.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    a11y(&resp, egui::Role::Switch, label, Some(*on));
    let p = pal(ui);
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let radius = rect.height() / 2.0;
    if *on {
        ui.painter().rect_filled(rect, radius, p.accent);
    } else {
        ui.painter().rect_stroke(rect, radius, Stroke::new(1.0, p.text_muted), StrokeKind::Inside);
    }
    focus_ring(ui, &resp, rect.expand(3.0), radius + 3.0);
    let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
    let knob = if *on { p.on_accent } else { p.text_muted };
    ui.painter().circle_filled(Pos2::new(x, rect.center().y), radius - if resp.hovered() { 4.0 } else { 5.0 }, knob);
    resp
}

/// Riga delle impostazioni: etichetta + descrizione a sinistra, controllo a destra.
pub fn setting_row(ui: &mut egui::Ui, label: &str, help: &str, control: impl FnOnce(&mut egui::Ui)) {
    let p = pal(ui);
    ui.horizontal(|ui| {
        ui.set_min_height(46.0);
        ui.vertical(|ui| {
            // La descrizione non deve mai finire sotto il controllo a destra.
            ui.set_max_width(ui.available_width() * 0.56);
            ui.add_space(4.0);
            ui.label(egui::RichText::new(label).size(14.0).color(p.text));
            if !help.is_empty() {
                ui.label(egui::RichText::new(help).size(12.5).color(p.text_muted));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control);
    });
}

/// Titolo di una sezione delle impostazioni.
pub fn settings_heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(14.0);
    ui.label(egui::RichText::new(text).size(15.0).strong().color(pal(ui).text));
    ui.add_space(2.0);
}

/// Pulsante con testo, opzionalmente "primario" (pieno d'accento).
pub fn button(ui: &mut egui::Ui, label: &str, primary: bool) -> Response {
    let p = pal(ui);
    let font = FontId::new(14.0, theme::family(primary, false));
    // Il colore va deciso prima dell'impaginazione: il testo impaginato lo tiene.
    let fg = if primary { p.on_accent } else { p.text };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, fg);
    let size = Vec2::new(galley.size().x + 28.0, 34.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    a11y(&resp, egui::Role::Button, label, None);
    let fill = if primary {
        if resp.hovered() { p.accent.gamma_multiply(0.85) } else { p.accent }
    } else if resp.hovered() {
        p.hover.gamma_multiply(2.2)
    } else {
        p.hover.gamma_multiply(1.4)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    if !primary {
        ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    }
    if resp.has_focus() {
        ui.painter().rect_stroke(rect.expand(2.0), 10.0, Stroke::new(1.5, p.accent_text), StrokeKind::Outside);
    }
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, fg);
    resp
}

/// Accorcia un testo lungo tenendo inizio e fine (es. percorsi): `C:/Users/…/Note`.
pub fn middle_ellipsis(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_owned();
    }
    let head = max * 2 / 5;
    let tail = max - head - 1;
    let start: String = s.chars().take(head).collect();
    let end: String = s.chars().skip(n - tail).collect();
    format!("{start}…{end}")
}

/// Finestra modale con intestazione propria: titolo al centro e una X che si
/// evidenzia al passaggio del mouse. Restituisce `true` se va chiusa
/// (X, Esc o clic fuori).
pub fn dialog(ctx: &egui::Context, id: &str, title: &str, width: f32, add: impl FnOnce(&mut egui::Ui)) -> bool {
    let p = theme::palette(ctx);
    let mut close = false;
    let frame = egui::Frame::window(&ctx.global_style()).inner_margin(0).corner_radius(14);
    let resp = egui::Modal::new(Id::unique(("yonote-dialog", id)))
        .frame(frame)
        .backdrop_color(Color32::from_black_alpha(if p.dark { 120 } else { 60 }))
        .show(ctx, |ui| {
            ui.set_width(width);
            // Intestazione.
            let (bar, _) = ui.allocate_exact_size(Vec2::new(width, 50.0), Sense::hover());
            ui.painter().text(bar.center(), Align2::CENTER_CENTER, title, FontId::new(15.0, theme::family(true, false)), p.text);
            let x = Rect::from_center_size(Pos2::new(bar.max.x - 26.0, bar.center().y), Vec2::splat(30.0));
            let r = ui.interact(x, Id::unique(("yonote-dialog-close", id)), Sense::click());
            a11y(&r, egui::Role::Button, crate::i18n::tr("Chiudi", "Close"), None);
            if r.hovered() || r.has_focus() {
                ui.painter().rect_filled(x, 7.0, p.hover.gamma_multiply(2.4));
                ui.painter().rect_stroke(x, 7.0, Stroke::new(1.0, p.border.gamma_multiply(2.0)), StrokeKind::Inside);
            }
            let icon_color = if r.hovered() { p.text } else { p.text_muted };
            icons::paint(ui, Icon::Close, Rect::from_center_size(x.center(), Vec2::splat(15.0)), icon_color);
            if r.on_hover_text(crate::i18n::tr("Chiudi (Esc)", "Close (Esc)")).clicked() {
                close = true;
            }
            ui.painter().hline(bar.x_range(), bar.max.y, Stroke::new(1.0, p.border));
            egui::Frame::NONE.inner_margin(egui::Margin { left: 20, right: 20, top: 16, bottom: 18 }).show(ui, |ui| {
                ui.set_width(width - 40.0);
                add(ui);
            });
        });
    close || resp.should_close()
}

/// Pulsante rosso per le azioni distruttive.
pub fn danger_button(ui: &mut egui::Ui, label: &str) -> Response {
    let p = pal(ui);
    let font = FontId::new(14.0, theme::family(true, false));
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, Color32::WHITE);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 28.0, 34.0), Sense::click());
    a11y(&resp, egui::Role::Button, label, None);
    let base = Color32::from_rgb(0xC4, 0x2B, 0x1C);
    let fill = if resp.hovered() { Color32::from_rgb(0xA8, 0x22, 0x15) } else { base };
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    if resp.has_focus() {
        ui.painter().rect_stroke(rect.expand(2.0), 10.0, Stroke::new(1.5, p.accent_text), StrokeKind::Outside);
    }
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
    resp
}

/// Colori di Ko-fi.
const KOFI_RED: Color32 = Color32::from_rgb(0xFF, 0x5E, 0x5B);
const KOFI_BLUE: Color32 = Color32::from_rgb(0x13, 0xC3, 0xFF);

/// Tazza di caffè con cuore, nello stile del pulsante di Ko-fi.
fn paint_kofi_cup(ui: &egui::Ui, r: Rect, cup: Color32, heart: Color32) {
    let painter = ui.painter();
    let w = r.width();
    let at = |x: f32, y: f32| Pos2::new(r.min.x + x * w, r.min.y + y * w);
    // Corpo della tazza.
    let body = Rect::from_min_max(at(0.12, 0.28), at(0.72, 0.86));
    painter.rect_filled(body, CornerRadius { nw: 3, ne: 3, sw: 10, se: 10 }, cup);
    // Manico.
    painter.circle_stroke(at(0.76, 0.5), w * 0.12, Stroke::new(w * 0.07, cup));
    // Cuore al centro (due cerchi e un triangolo).
    let c = body.center() + Vec2::new(0.0, -w * 0.02);
    let s = w * 0.085;
    painter.circle_filled(c + Vec2::new(-s * 0.9, -s * 0.2), s, heart);
    painter.circle_filled(c + Vec2::new(s * 0.9, -s * 0.2), s, heart);
    painter.add(egui::Shape::convex_polygon(
        vec![c + Vec2::new(-s * 1.85, 0.1 * s), c + Vec2::new(s * 1.85, 0.1 * s), c + Vec2::new(0.0, s * 2.0)],
        heart,
        Stroke::NONE,
    ));
}

/// Scheda per le donazioni su Ko-fi. Restituisce `true` se si clicca il pulsante.
pub fn kofi_card(ui: &mut egui::Ui) -> bool {
    use crate::i18n::tr;
    let p = pal(ui);
    let mut clicked = false;
    let fill = if p.dark { Color32::from_rgb(0x33, 0x24, 0x26) } else { Color32::from_rgb(0xFF, 0xF1, 0xF0) };
    egui::Frame::NONE.fill(fill).stroke(Stroke::new(1.0, KOFI_RED.gamma_multiply(0.45))).corner_radius(12).inner_margin(16).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
                paint_kofi_cup(ui, r, KOFI_RED, Color32::WHITE);
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width() - 190.0);
                    ui.label(
                        egui::RichText::new(tr("Ti piace Yonote?", "Enjoying Yonote?"))
                            .size(15.0)
                            .family(theme::family(true, false))
                            .color(p.text),
                    );
                    ui.label(
                        egui::RichText::new(tr(
                            "È gratis e lo resterà. Un caffè su Ko-fi aiuta a farlo crescere.",
                            "It's free and always will be. A coffee on Ko-fi helps it grow.",
                        ))
                        .size(12.5)
                        .color(p.text_muted),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = tr("Supporta su Ko-fi", "Support on Ko-fi");
                    let font = FontId::new(14.0, theme::family(true, false));
                    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, Color32::WHITE);
                    let size = Vec2::new(galley.size().x + 56.0, 38.0);
                    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                    a11y(&resp, egui::Role::Link, label, None);
                    focus_ring(ui, &resp, rect, 19.0);
                    let bg = if resp.hovered() { Color32::from_rgb(0x00, 0xA8, 0xE4) } else { KOFI_BLUE };
                    ui.painter().rect_filled(rect, 19.0, bg);
                    let cup = Rect::from_center_size(Pos2::new(rect.min.x + 22.0, rect.center().y), Vec2::splat(22.0));
                    paint_kofi_cup(ui, cup, Color32::WHITE, KOFI_RED);
                    ui.painter().galley(Pos2::new(rect.min.x + 40.0, rect.center().y - galley.size().y / 2.0), galley, Color32::WHITE);
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.on_hover_text("ko-fi.com/moosedevpjs").clicked() {
                        clicked = true;
                    }
                });
            });
        },
    );
    clicked
}

#[cfg(test)]
mod tests {
    #[test]
    fn middle_ellipsis_keeps_both_ends() {
        assert_eq!(super::middle_ellipsis("corto", 10), "corto");
        let s = super::middle_ellipsis("C:/Users/gabri/AppData/Local/Temp/vault", 20);
        assert_eq!(s.chars().count(), 20);
        assert!(s.starts_with("C:/Users") && s.ends_with("Temp/vault"));
    }
}
