//! Funzioni specifiche di Windows: sfondo Mica/Acrylic sempre attivo e
//! registrazione di Blocks in "Apri con" per i file Markdown.

use std::sync::atomic::{AtomicBool, Ordering};

/// Se vero, la finestra si disegna sempre come "attiva": Windows spegne
/// Mica/Acrylic sulle finestre senza focus, così invece l'effetto resta.
static KEEP_ACTIVE: AtomicBool = AtomicBool::new(false);
static SUBCLASSED: AtomicBool = AtomicBool::new(false);

pub fn hwnd(window: &impl raw_window_handle::HasWindowHandle) -> Option<isize> {
    match window.window_handle().ok()?.as_raw() {
        raw_window_handle::RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}

/// Mantiene (o no) lo sfondo trasparente quando la finestra perde il focus.
pub fn keep_backdrop_when_inactive(hwnd: isize, keep: bool) {
    use windows_sys::Win32::UI::Shell::SetWindowSubclass;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_NCACTIVATE};
    KEEP_ACTIVE.store(keep, Ordering::Relaxed);
    let h = hwnd as windows_sys::Win32::Foundation::HWND;
    if !SUBCLASSED.swap(true, Ordering::Relaxed) {
        // SAFETY: `h` è la finestra principale di questo processo, valida finché l'app è aperta.
        unsafe {
            SetWindowSubclass(h, Some(subclass_proc), 1, 0);
        }
    }
    // Ridisegna subito la cornice con lo stato giusto.
    // SAFETY: messaggio standard alla nostra finestra.
    unsafe {
        let active = windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() == h;
        SendMessageW(h, WM_NCACTIVATE, usize::from(active || keep), 0);
    }
}

unsafe extern "system" fn subclass_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
    _id: usize,
    _data: usize,
) -> isize {
    use windows_sys::Win32::UI::Shell::DefSubclassProc;
    use windows_sys::Win32::UI::WindowsAndMessaging::WM_NCACTIVATE;
    // Il focus (WM_ACTIVATE/WM_SETFOCUS) resta invariato: cambia solo il disegno della cornice,
    // da cui Windows decide se mostrare Mica/Acrylic.
    let wparam = if msg == WM_NCACTIVATE && KEEP_ACTIVE.load(Ordering::Relaxed) { 1 } else { wparam };
    // SAFETY: inoltra il messaggio alla procedura originale della finestra.
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Apre una cartella in Esplora file.
pub fn open_folder(dir: &std::path::Path) {
    let _ = std::process::Command::new("explorer").arg(dir).spawn();
}

/// Mostra un file selezionato in Esplora file.
pub fn reveal_file(file: &std::path::Path) {
    let _ = std::process::Command::new("explorer").arg(format!("/select,{}", file.display())).spawn();
}

// ---------------------------------------------------------------------------
// "Apri con"

const PROG_ID: &str = "Blocks.Markdown";
const EXTENSIONS: [&str; 2] = [".md", ".markdown"];

fn exe_path() -> std::io::Result<String> {
    Ok(std::env::current_exe()?.display().to_string())
}

fn notify_shell() {
    use windows_sys::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
    // SAFETY: notifica standard a Esplora file che le associazioni sono cambiate.
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED as i32, SHCNF_IDLIST, std::ptr::null(), std::ptr::null()) };
}

/// Aggiunge Blocks a "Apri con" per i file Markdown (solo per l'utente corrente).
/// Il file viene aperto e modificato direttamente, senza copie.
pub fn register_open_with() -> std::io::Result<()> {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let exe = exe_path()?;
    let command = format!("\"{exe}\" \"%1\"");
    let classes = RegKey::predef(HKEY_CURRENT_USER).create_subkey("Software\\Classes")?.0;

    let (prog, _) = classes.create_subkey(PROG_ID)?;
    prog.set_value("", &"Nota Markdown (Blocks)")?;
    prog.create_subkey("DefaultIcon")?.0.set_value("", &format!("\"{exe}\",0"))?;
    prog.create_subkey("shell\\open\\command")?.0.set_value("", &command)?;

    let (app, _) = classes.create_subkey("Applications\\blocks.exe")?;
    app.set_value("FriendlyAppName", &"Blocks")?;
    app.create_subkey("shell\\open\\command")?.0.set_value("", &command)?;
    let (types, _) = app.create_subkey("SupportedTypes")?;
    for ext in EXTENSIONS {
        types.set_value(ext, &"")?;
        classes.create_subkey(format!("{ext}\\OpenWithProgids"))?.0.set_value(PROG_ID, &"")?;
    }
    notify_shell();
    Ok(())
}

pub fn unregister_open_with() -> std::io::Result<()> {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let classes = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags("Software\\Classes", winreg::enums::KEY_ALL_ACCESS)?;
    let ignore_missing = |r: std::io::Result<()>| match r {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    };
    ignore_missing(classes.delete_subkey_all(PROG_ID))?;
    ignore_missing(classes.delete_subkey_all("Applications\\blocks.exe"))?;
    for ext in EXTENSIONS {
        if let Ok(k) = classes.open_subkey_with_flags(format!("{ext}\\OpenWithProgids"), winreg::enums::KEY_ALL_ACCESS) {
            ignore_missing(k.delete_value(PROG_ID))?;
        }
    }
    notify_shell();
    Ok(())
}

/// Blocks è registrato in "Apri con" con questo eseguibile?
pub fn is_registered() -> bool {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let Ok(exe) = exe_path() else { return false };
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(format!("Software\\Classes\\{PROG_ID}\\shell\\open\\command"))
        .and_then(|k| k.get_value::<String, _>(""))
        .is_ok_and(|cmd| cmd.to_lowercase().contains(&exe.to_lowercase()))
}
