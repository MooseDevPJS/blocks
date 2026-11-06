// Niente finestra della console nelle build di rilascio.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod editor;
mod fsutil;
mod i18n;
mod icons;
mod markdown;
mod platform;
mod settings;
mod theme;
mod vault;
mod widgets;

/// Icona dell'app (stessa forma del marchio disegnato nell'interfaccia).
pub const APP_ICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect x="2" y="2" width="60" height="60" rx="15" fill="#E09F1F"/><g stroke="#fff" stroke-width="4.8" stroke-linecap="round"><path d="M18 22h28M18 32h28M18 42h17"/></g></svg>"##;

fn app_icon() -> Option<eframe::egui::IconData> {
    let tree = resvg::usvg::Tree::from_str(APP_ICON_SVG, &resvg::usvg::Options::default()).ok()?;
    let size = 256;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)?;
    let scale = size as f32 / tree.size().width();
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    // tiny-skia usa alfa premoltiplicato: riportalo a normale per l'icona.
    let rgba = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    Some(eframe::egui::IconData { rgba, width: size, height: size })
}

fn main() -> eframe::Result {
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Blocks")
        .with_inner_size([1240.0, 800.0])
        .with_min_inner_size([480.0, 360.0])
        .with_transparent(true);
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    let options = eframe::NativeOptions {
        viewport,
        // OpenGL gestisce bene la trasparenza della finestra su Windows (serve a Mica/Acrylic).
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    // Un file passato all'avvio (doppio clic, "Apri con", trascinato sull'icona).
    let file = std::env::args_os().nth(1).map(std::path::PathBuf::from).filter(|p| p.is_file());
    eframe::run_native("Blocks", options, Box::new(move |cc| Ok(Box::new(app::BlocksApp::new(cc, file)))))
}
