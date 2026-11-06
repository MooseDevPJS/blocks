//! Impostazioni dell'utente, salvate in `%APPDATA%\Blocks\config\settings.toml`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

/// Effetto di sfondo della finestra (materiali di Windows 11).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Backdrop {
    /// Sfondo pieno.
    Off,
    /// Mica: riprende il colore dello sfondo del desktop, molto leggibile.
    #[default]
    Mica,
    /// Acrylic: vetro smerigliato, si intravede cosa c'è dietro la finestra.
    Acrylic,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Language {
    #[default]
    Auto,
    It,
    En,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Cartella delle note. `None` = primo avvio.
    pub vault: Option<PathBuf>,
    pub theme: ThemeMode,
    pub backdrop: Backdrop,
    /// Opacità dei pannelli sopra lo sfondo trasparente (0.0–1.0).
    pub backdrop_opacity: f32,
    /// Colore d'accento in sRGB.
    pub accent: [u8; 3],
    pub language: Language,
    /// Dimensione del testo dell'editor, in punti.
    pub font_size: f32,
    /// Interlinea (multiplo della dimensione del testo).
    pub line_spacing: f32,
    /// Larghezza massima della colonna di testo, in punti.
    pub text_width: f32,
    /// Font del testo: "" = Segoe UI di sistema, altrimenti nome del file in C:\Windows\Fonts.
    pub editor_font: String,
    /// Nome del tema da file (`themes/<nome>.toml`), vuoto = nessuno.
    pub custom_theme: String,
    pub sidebar_open: bool,
    pub list_open: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            vault: None,
            theme: ThemeMode::System,
            backdrop: Backdrop::Mica,
            backdrop_opacity: 0.72,
            accent: crate::theme::ACCENTS[0].1,
            language: Language::Auto,
            font_size: 16.0,
            line_spacing: 1.55,
            text_width: 720.0,
            editor_font: String::new(),
            custom_theme: String::new(),
            sidebar_open: true,
            list_open: true,
        }
    }
}

impl Settings {
    /// Cartella di impostazioni, temi e log. `BLOCKS_CONFIG_DIR` la sostituisce
    /// (versione portatile, test).
    pub fn dir() -> Option<PathBuf> {
        if let Some(d) = std::env::var_os("BLOCKS_CONFIG_DIR").filter(|d| !d.is_empty()) {
            return Some(PathBuf::from(d));
        }
        directories::ProjectDirs::from("", "", "Blocks").map(|d| d.config_dir().to_path_buf())
    }

    fn path() -> Option<PathBuf> {
        Self::dir().map(|d| d.join("settings.toml"))
    }

    /// Carica le impostazioni; se il file manca o è rovinato usa quelle predefinite.
    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| toml::from_str(s.trim_start_matches('\u{feff}')).ok())
            .map(Self::clamped)
            .unwrap_or_default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path().ok_or_else(|| anyhow::anyhow!("cartella impostazioni non trovata"))?;
        crate::fsutil::write_atomic(&path, toml::to_string_pretty(self)?.as_bytes())?;
        Ok(())
    }

    fn clamped(mut self) -> Self {
        self.font_size = self.font_size.clamp(10.0, 32.0);
        self.text_width = self.text_width.clamp(400.0, 2000.0);
        self.line_spacing = self.line_spacing.clamp(1.1, 2.2);
        self.backdrop_opacity = self.backdrop_opacity.clamp(0.3, 1.0);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_or_partial_files_still_load() {
        let s: Settings = toml::from_str("vault = \"C:/Note\"\ntheme = \"Dark\"").unwrap();
        assert_eq!(s.theme, ThemeMode::Dark);
        assert_eq!(s.backdrop, Backdrop::Mica);
        let out = toml::to_string_pretty(&Settings::default()).unwrap();
        let back: Settings = toml::from_str(&out).unwrap();
        assert_eq!(back, Settings::default());
    }
}
