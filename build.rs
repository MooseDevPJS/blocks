//! Mette nell'eseguibile l'icona dell'app e le informazioni sulla versione
//! (quelle che Windows mostra in Esplora file e nelle Proprietà).
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/blocks.ico")
            .set("ProductName", "Blocks")
            .set("FileDescription", "Blocks — light Markdown notes")
            .set("LegalCopyright", "© 2026 MooseDevPJS · PolyForm Noncommercial 1.0.0");
        if let Err(e) = res.compile() {
            println!("cargo:warning=icona non inclusa: {e}");
        }
    }
    println!("cargo:rerun-if-changed=assets/blocks.ico");
}
