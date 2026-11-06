//! Scrittura sicura su disco.

use std::fs;
use std::io::Write;
use std::path::Path;

/// Scrive `bytes` in `path` senza mai lasciare un file a metà:
/// scrive un file temporaneo accanto all'originale, lo forza su disco e poi
/// lo rinomina sopra l'originale (operazione atomica sullo stesso volume).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.tmp"));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    // Su Windows `rename` sostituisce il file di destinazione se esiste.
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}
