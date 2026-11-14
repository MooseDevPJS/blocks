//! Aggiornamenti dall'ultima release su GitHub.
//!
//! Il controllo e il download girano in un thread a parte e usano `curl.exe`
//! di Windows (certificati di sistema, nessuna libreria di rete in più).
//! L'installer scaricato viene verificato con l'impronta SHA-256 pubblicata da
//! GitHub e avviato in modalità silenziosa: chiude Yonote, si aggiorna e la riapre.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Repository da cui arrivano gli aggiornamenti.
pub const REPO: &str = "MooseDevPJS/yonote";
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    /// Versione senza la "v" iniziale (es. "0.2.1").
    pub version: String,
    /// Testo della release (Markdown).
    pub notes: String,
    /// Pagina della release su GitHub.
    pub page: String,
    pub asset_url: String,
    pub size: u64,
    /// Impronta SHA-256 in esadecimale, se GitHub la fornisce.
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Status {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    /// Download in corso: avanzamento da 0 a 1.
    Downloading(Release, f32),
    /// Installer scaricato e verificato, pronto da avviare.
    Ready(Release, PathBuf),
    Failed(String),
}

/// Stato degli aggiornamenti condiviso con i thread di lavoro.
#[derive(Clone, Default)]
pub struct Updater {
    status: Arc<Mutex<Status>>,
}

impl Updater {
    pub fn status(&self) -> Status {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    fn set(&self, s: Status) {
        if let Ok(mut st) = self.status.lock() {
            *st = s;
        }
    }

    pub fn busy(&self) -> bool {
        matches!(self.status(), Status::Checking | Status::Downloading(..))
    }

    /// Cerca una nuova versione (non fa nulla se un controllo o un download sono già in corso).
    pub fn check(&self, ctx: &eframe::egui::Context) {
        if self.busy() {
            return;
        }
        self.set(Status::Checking);
        let (me, ctx) = (self.clone(), ctx.clone());
        std::thread::spawn(move || {
            let result = match fetch_latest() {
                Ok(r) if is_newer(&r.version, CURRENT) => Status::Available(r),
                Ok(_) => Status::UpToDate,
                Err(e) => Status::Failed(e),
            };
            me.set(result);
            ctx.request_repaint();
        });
    }

    /// Scarica l'installer della release e lo verifica.
    pub fn download(&self, ctx: &eframe::egui::Context, rel: Release) {
        if self.busy() {
            return;
        }
        self.set(Status::Downloading(rel.clone(), 0.0));
        let (me, ctx) = (self.clone(), ctx.clone());
        std::thread::spawn(move || {
            let result = match download_installer(&rel, |p| {
                me.set(Status::Downloading(rel.clone(), p));
                ctx.request_repaint();
            }) {
                Ok(path) => Status::Ready(rel, path),
                Err(e) => Status::Failed(e),
            };
            me.set(result);
            ctx.request_repaint();
        });
    }

    pub fn dismiss_error(&self) {
        if matches!(self.status(), Status::Failed(_)) {
            self.set(Status::Idle);
        }
    }
}

/// `curl.exe` di Windows, senza finestra della console.
fn curl() -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let system = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| "C:\\Windows".into());
    let mut c = Command::new(system.join("System32").join("curl.exe"));
    c.creation_flags(CREATE_NO_WINDOW);
    c.args([
        "--silent",
        "--show-error",
        "--fail",
        "--location",
        "--max-time",
        "600",
        "--user-agent",
        concat!("Yonote/", env!("CARGO_PKG_VERSION")),
    ]);
    c
}

fn fetch_latest() -> Result<Release, String> {
    let out = curl()
        .args(["--header", "Accept: application/vnd.github+json"])
        .arg(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        crate::app::log_error(&format!("update check: {}", String::from_utf8_lossy(&out.stderr).trim()));
        return Err(crate::i18n::tr("impossibile contattare GitHub", "couldn't reach GitHub").to_owned());
    }
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    parse_release(&json).ok_or_else(|| crate::i18n::tr("la release non contiene l'installer", "the release has no installer").to_owned())
}

/// Estrae dalla risposta di GitHub la versione e l'installer (`…-setup.exe`).
pub fn parse_release(json: &serde_json::Value) -> Option<Release> {
    let version = json["tag_name"].as_str()?.trim_start_matches('v').to_owned();
    let asset = json["assets"].as_array()?.iter().find(|a| a["name"].as_str().is_some_and(|n| n.ends_with("-setup.exe")))?;
    let asset_url = asset["browser_download_url"].as_str()?.to_owned();
    // Si scarica solo dalle release di questo repository.
    if !asset_url.starts_with(&format!("https://github.com/{REPO}/releases/download/")) {
        return None;
    }
    Some(Release {
        version,
        notes: json["body"].as_str().unwrap_or("").to_owned(),
        page: json["html_url"].as_str().unwrap_or("").to_owned(),
        asset_url,
        size: asset["size"].as_u64().unwrap_or(0),
        sha256: asset["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")).map(str::to_lowercase),
    })
}

/// `remote` è più recente di `current`? (Confronto per numeri: 0.10.0 > 0.9.3.)
pub fn is_newer(remote: &str, current: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v').split(['-', '+']).next().unwrap_or("").split('.').map(|n| n.parse().unwrap_or(0)).collect()
    };
    let (r, c) = (parts(remote), parts(current));
    for i in 0..r.len().max(c.len()) {
        let (a, b) = (r.get(i).copied().unwrap_or(0), c.get(i).copied().unwrap_or(0));
        if a != b {
            return a > b;
        }
    }
    false
}

fn download_installer(rel: &Release, progress: impl Fn(f32)) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("Yonote-update");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("Yonote-{}-setup.exe", rel.version));
    let _ = std::fs::remove_file(&path);
    let mut child = curl().arg("--output").arg(&path).arg(&rel.asset_url).spawn().map_err(|e| e.to_string())?;
    let status = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        if rel.size > 0 {
            let done = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            progress((done as f32 / rel.size as f32).min(0.99));
        }
        std::thread::sleep(Duration::from_millis(120));
    };
    if !status.success() {
        return Err(crate::i18n::tr("download non riuscito", "download failed").to_owned());
    }
    verify(&path, rel)?;
    progress(1.0);
    Ok(path)
}

/// Controlla dimensione e impronta SHA-256 del file scaricato.
fn verify(path: &Path, rel: &Release) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let bad = |what: &str| {
        let _ = std::fs::remove_file(path);
        Err(format!("{} ({what})", crate::i18n::tr("il file scaricato non è valido", "the downloaded file is not valid")))
    };
    if rel.size > 0 && bytes.len() as u64 != rel.size {
        return bad("size");
    }
    if let Some(expected) = &rel.sha256 {
        let actual: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        if &actual != expected {
            return bad("SHA-256");
        }
    }
    Ok(())
}

/// Avvia l'installer in modalità silenziosa: chiude Yonote, la aggiorna e la riapre.
pub fn run_installer(path: &Path) -> std::io::Result<()> {
    Command::new(path).args(["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS"]).spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("0.2.0", "0.1.1"));
        assert!(is_newer("v0.10.0", "0.9.3"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("0.2.0-beta", "0.2.0"));
    }

    #[test]
    fn release_json_is_parsed_and_checked() {
        let json = serde_json::json!({
            "tag_name": "v0.3.0",
            "html_url": "https://github.com/MooseDevPJS/yonote/releases/tag/v0.3.0",
            "body": "News",
            "assets": [
                {"name": "notes.txt", "browser_download_url": "https://github.com/MooseDevPJS/yonote/releases/download/v0.3.0/notes.txt", "size": 1},
                {"name": "Yonote-0.3.0-setup.exe", "browser_download_url": "https://github.com/MooseDevPJS/yonote/releases/download/v0.3.0/Yonote-0.3.0-setup.exe", "size": 42, "digest": "sha256:ABC"}
            ]
        });
        let r = parse_release(&json).unwrap();
        assert_eq!(r.version, "0.3.0");
        assert_eq!(r.size, 42);
        assert_eq!(r.sha256.as_deref(), Some("abc"));
        // Un installer che non arriva da questo repository viene ignorato.
        let mut evil = json.clone();
        evil["assets"][1]["browser_download_url"] = "https://example.com/Yonote-0.3.0-setup.exe".into();
        assert!(parse_release(&evil).is_none());
    }

    #[test]
    fn verify_rejects_wrong_hash() {
        let p = std::env::temp_dir().join(format!("yonote-verify-{}", std::process::id()));
        std::fs::write(&p, b"hello").unwrap();
        let mut rel =
            Release { version: "1".into(), notes: String::new(), page: String::new(), asset_url: String::new(), size: 5, sha256: None };
        rel.sha256 = Some("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".into());
        assert!(verify(&p, &rel).is_ok());
        rel.sha256 = Some("00".into());
        assert!(verify(&p, &rel).is_err());
        assert!(!p.exists());
    }
}
