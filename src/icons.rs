//! Icone vettoriali disegnate a mano (SVG, griglia 24, tratto 1.75, angoli arrotondati).
//!
//! Sono bianche nel sorgente e vengono colorate al momento del disegno,
//! così seguono tema e colore d'accento.

use eframe::egui::{self, Color32, Image, ImageSource, Response, Sense, Vec2};

macro_rules! svg {
    ($name:literal, $body:literal) => {
        ImageSource::Bytes {
            uri: std::borrow::Cow::Borrowed(concat!("bytes://icon-", $name, ".svg")),
            bytes: egui::load::Bytes::Static(
                concat!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
                    $body,
                    "</svg>"
                )
                .as_bytes(),
            ),
        }
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Compose,
    Search,
    Notes,
    Folder,
    FolderOpen,
    FolderPlus,
    Hash,
    Pin,
    Trash,
    Settings,
    Sidebar,
    Close,
    ChevronRight,
    External,
    Link,
    Palette,
    Update,
}

impl Icon {
    pub fn source(self) -> ImageSource<'static> {
        match self {
            Icon::Compose => svg!(
                "compose",
                r#"<path d="M11 4H6.5A2.5 2.5 0 0 0 4 6.5v11A2.5 2.5 0 0 0 6.5 20h11a2.5 2.5 0 0 0 2.5-2.5V13"/><path d="M17.6 3.9a1.9 1.9 0 0 1 2.7 2.7L12.5 14.4 9 15l.6-3.5z"/>"#
            ),
            Icon::Search => svg!("search", r#"<circle cx="10.5" cy="10.5" r="6.5"/><path d="m20 20-4.6-4.6"/>"#),
            Icon::Notes => {
                svg!("notes", r#"<rect x="4.5" y="3.5" width="15" height="17" rx="2.5"/><path d="M8.5 8.5h7M8.5 12h7M8.5 15.5h4"/>"#)
            }
            Icon::Folder => svg!(
                "folder",
                r#"<path d="M3.5 7.5A2 2 0 0 1 5.5 5.5h3.6a2 2 0 0 1 1.5.7l1.2 1.4a2 2 0 0 0 1.5.7h5.2a2 2 0 0 1 2 2v7.2a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"/>"#
            ),
            Icon::FolderOpen => svg!(
                "folder-open",
                r#"<path d="M3.5 16.5V7.5a2 2 0 0 1 2-2h3.6a2 2 0 0 1 1.5.7l1.2 1.4a2 2 0 0 0 1.5.7h4.2a2 2 0 0 1 2 2v.7"/><path d="M3.6 17.4 6 11.9a2 2 0 0 1 1.8-1.2h11.9a1.3 1.3 0 0 1 1.2 1.8l-2.3 5.3a2 2 0 0 1-1.8 1.2H4.8a1.3 1.3 0 0 1-1.2-1.6z"/>"#
            ),
            Icon::FolderPlus => svg!(
                "folder-plus",
                r#"<path d="M20.5 12V10.3a2 2 0 0 0-2-2h-5.2a2 2 0 0 1-1.5-.7l-1.2-1.4a2 2 0 0 0-1.5-.7H5.5a2 2 0 0 0-2 2v9.2a2 2 0 0 0 2 2H12"/><path d="M18 15v6M15 18h6"/>"#
            ),
            Icon::Hash => svg!("hash", r#"<path d="M9.5 4 8 20M16 4l-1.5 16M4.5 9h16M3.5 15h16"/>"#),
            Icon::Pin => svg!(
                "pin",
                r#"<path d="M9 3.5h6M10 3.5v5.2L6.8 12.6a1 1 0 0 0 .8 1.6h8.8a1 1 0 0 0 .8-1.6L14 8.7V3.5"/><path d="M12 14.2v6.3"/>"#
            ),
            Icon::Trash => svg!(
                "trash",
                r#"<path d="M4 6.5h16M9.5 6.5V4.8a1.3 1.3 0 0 1 1.3-1.3h2.4a1.3 1.3 0 0 1 1.3 1.3v1.7"/><path d="m6 6.5.9 12.1A2 2 0 0 0 8.9 20.5h6.2a2 2 0 0 0 2-1.9L18 6.5M10 10.5v6M14 10.5v6"/>"#
            ),
            Icon::Settings => {
                svg!("settings", r#"<path d="M4 7h9M17 7h3M4 17h3M11 17h9"/><circle cx="15" cy="7" r="2"/><circle cx="9" cy="17" r="2"/>"#)
            }
            Icon::Sidebar => svg!("sidebar", r#"<rect x="3.5" y="4.5" width="17" height="15" rx="2.5"/><path d="M9.5 4.5v15"/>"#),
            Icon::Close => svg!("close", r#"<path d="M6.5 6.5l11 11M17.5 6.5l-11 11"/>"#),
            Icon::ChevronRight => svg!("chevron-right", r#"<path d="m9.5 6 6 6-6 6"/>"#),
            Icon::External => svg!(
                "external",
                r#"<path d="M13.5 4.5h6v6M19.5 4.5 11 13"/><path d="M17.5 14v3.5a2 2 0 0 1-2 2h-9a2 2 0 0 1-2-2v-9a2 2 0 0 1 2-2H10"/>"#
            ),
            Icon::Link => svg!(
                "link",
                r#"<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1"/><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>"#
            ),
            Icon::Update => svg!(
                "update",
                r#"<path d="M12 15.5v-10M7.5 10 12 5.5l4.5 4.5"/><path d="M5 15.5v1.5a2.5 2.5 0 0 0 2.5 2.5h9a2.5 2.5 0 0 0 2.5-2.5v-1.5"/>"#
            ),
            Icon::Palette => svg!(
                "palette",
                r#"<path d="M12 3.5a8.5 8.5 0 0 0 0 17c1.2 0 1.8-.8 1.8-1.7 0-.5-.2-.9-.5-1.3-.3-.3-.5-.7-.5-1.2 0-.9.8-1.8 1.8-1.8h2.1a3.8 3.8 0 0 0 3.8-3.8C20.5 6.9 16.7 3.5 12 3.5z"/><circle cx="7.8" cy="11.5" r=".9"/><circle cx="10.3" cy="7.6" r=".9"/><circle cx="14.8" cy="7.8" r=".9"/>"#
            ),
        }
    }
}

/// La Y del logo di Yonote (bianca nel sorgente, colorata al momento del disegno).
pub fn logo_glyph() -> ImageSource<'static> {
    ImageSource::Bytes {
        uri: std::borrow::Cow::Borrowed("bytes://yonote-glyph.svg"),
        bytes: egui::load::Bytes::Static(include_bytes!("../assets/yonote-glyph.svg")),
    }
}

/// Disegna un'icona nel rettangolo indicato.
pub fn paint(ui: &egui::Ui, icon: Icon, rect: egui::Rect, color: Color32) {
    Image::new(icon.source()).tint(color).paint_at(ui, rect);
}

/// Icona statica nel flusso dell'interfaccia.
pub fn show(ui: &mut egui::Ui, icon: Icon, size: f32, color: Color32) -> Response {
    ui.add(Image::new(icon.source()).tint(color).fit_to_exact_size(Vec2::splat(size)))
}

/// Pulsante con sola icona: area cliccabile comoda, sfondo al passaggio del mouse.
pub fn button(ui: &mut egui::Ui, icon: Icon, tooltip: &str) -> Response {
    button_sized(ui, icon, tooltip, 30.0, 18.0)
}

pub fn button_sized(ui: &mut egui::Ui, icon: Icon, tooltip: &str, box_size: f32, icon_size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(box_size), Sense::click());
    crate::widgets::a11y(&resp, egui::Role::Button, tooltip, None);
    let v = ui.visuals();
    let color = if resp.hovered() { v.strong_text_color() } else { v.text_color().gamma_multiply(0.78) };
    if resp.hovered() || resp.has_focus() {
        ui.painter().rect_filled(rect, 7.0, v.widgets.hovered.weak_bg_fill);
    }
    if resp.has_focus() {
        ui.painter().rect_stroke(rect, 7.0, egui::Stroke::new(1.5, v.selection.stroke.color), egui::StrokeKind::Inside);
    }
    paint(ui, icon, egui::Rect::from_center_size(rect.center(), Vec2::splat(icon_size)), color);
    resp.on_hover_text(tooltip)
}
