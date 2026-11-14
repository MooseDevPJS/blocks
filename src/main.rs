// Niente finestra della console nelle build di rilascio.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod editor;
mod find;
mod fsutil;
mod i18n;
mod icons;
mod markdown;
mod platform;
mod settings;
mod theme;
mod update;
mod vault;
mod widgets;

/// Icona della finestra e della barra delle applicazioni (la stessa dell'eseguibile).
fn app_icon() -> Option<eframe::egui::IconData> {
    let img = image::load_from_memory_with_format(include_bytes!("../assets/yonote-256.png"), image::ImageFormat::Png).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Some(eframe::egui::IconData { rgba: img.into_raw(), width, height })
}

fn main() -> eframe::Result {
    settings::Settings::migrate_legacy_dir();
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Yonote")
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
    eframe::run_native("Yonote", options, Box::new(move |cc| Ok(Box::new(app::YonoteApp::new(cc, file)))))
}
