//! Aspetto: font di Windows, palette chiaro/scuro, accento, sfondo trasparente
//! e temi da file `.toml`.

use crate::settings::{Backdrop, Settings};
use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, Id, Shadow, Stroke, Theme};
use serde::Deserialize;
use std::sync::Arc;

pub const BOLD: &str = "bold";
pub const ITALIC: &str = "italic";
pub const BOLD_ITALIC: &str = "bold-italic";

/// Colori d'accento proposti nelle impostazioni.
/// (nome italiano, nome inglese, colore). Il primo è quello predefinito.
pub const ACCENTS: [(&str, &str, [u8; 3]); 10] = [
    ("Bianco e nero", "Black & white", MONO),
    ("Ambra", "Amber", [0xE0, 0x9F, 0x1F]),
    ("Blu", "Blue", [0x2F, 0x7B, 0xE8]),
    ("Indaco", "Indigo", [0x63, 0x66, 0xF1]),
    ("Viola", "Violet", [0x9B, 0x5D, 0xE5]),
    ("Rosa", "Pink", [0xE5, 0x4F, 0x8E]),
    ("Corallo", "Coral", [0xEF, 0x6A, 0x4C]),
    ("Verde", "Green", [0x2E, 0xA0, 0x6B]),
    ("Petrolio", "Teal", [0x14, 0x9A, 0xA3]),
    ("Grafite", "Graphite", [0x6E, 0x77, 0x81]),
];

/// Accento "bianco e nero": nero nel tema chiaro, bianco nel tema scuro
/// (prende il colore del testo del tema, anche dei temi da file).
pub const MONO: [u8; 3] = [0, 0, 0];

pub fn family(bold: bool, italic: bool) -> FontFamily {
    match (bold, italic) {
        (false, false) => FontFamily::Proportional,
        (true, false) => FontFamily::Name(BOLD.into()),
        (false, true) => FontFamily::Name(ITALIC.into()),
        (true, true) => FontFamily::Name(BOLD_ITALIC.into()),
    }
}

fn fonts_dir() -> std::path::PathBuf {
    std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("Fonts")
}

/// Font dell'editor proposti nelle impostazioni: (nome, file regular, file bold, file italic).
pub const EDITOR_FONTS: [(&str, &str, &str, &str); 5] = [
    ("Segoe UI", "segoeui.ttf", "seguisb.ttf", "segoeuii.ttf"),
    ("Georgia", "georgia.ttf", "georgiab.ttf", "georgiai.ttf"),
    ("Cambria", "cambria.ttc", "cambriab.ttf", "cambriai.ttf"),
    ("Calibri", "calibri.ttf", "calibrib.ttf", "calibrii.ttf"),
    ("Cascadia Mono", "CascadiaMono.ttf", "CascadiaMono.ttf", "CascadiaMono.ttf"),
];

/// Usa i font installati in Windows (Segoe UI, Cascadia/Consolas) se presenti,
/// mantenendo i font di egui come riserva per simboli ed emoji.
/// `editor_font` sceglie il font del testo delle note (famiglie "editor*").
pub fn install_fonts(ctx: &egui::Context, editor_font: &str) {
    let mut fonts = FontDefinitions::default();
    let dir = fonts_dir();
    // Carica il primo file valido (un font rovinato viene ignorato, non fa chiudere l'app).
    let mut load = |key: &str, files: &[&str]| -> bool {
        for f in files {
            if let Ok(bytes) = std::fs::read(dir.join(f))
                && is_valid_font(&bytes)
            {
                fonts.font_data.insert(key.to_owned(), Arc::new(FontData::from_owned(bytes)));
                return true;
            }
        }
        false
    };
    let regular = load("ui", &["segoeui.ttf"]);
    let semibold = load("ui-bold", &["seguisb.ttf", "segoeuib.ttf"]);
    let italic = load("ui-italic", &["segoeuii.ttf"]);
    let semibold_italic = load("ui-bold-italic", &["seguisbi.ttf", "segoeuiz.ttf"]);
    let mono = load("mono", &["CascadiaMono.ttf", "consola.ttf"]);
    let symbols = load("symbols", &["seguisym.ttf"]);
    // Niente Segoe UI Emoji (12 MB): egui la disegnerebbe comunque in bianco e nero
    // e i suoi font di riserva contengono già le emoji.

    let files: Option<[String; 3]> = if let Some(name) = editor_font.strip_prefix(IMPORTED) {
        // Font importato: cerca anche le varianti con lo stesso nome (Inter-Bold.ttf…).
        user_fonts_dir().map(|d| {
            let path = d.join(name);
            let variant = |v: &str| sibling_variant(&path, v).unwrap_or_else(|| path.clone()).display().to_string();
            [path.display().to_string(), variant("Bold"), variant("Italic")]
        })
    } else {
        EDITOR_FONTS.iter().find(|f| f.0 == editor_font && f.0 != "Segoe UI").map(|f| [f.1.to_owned(), f.2.to_owned(), f.3.to_owned()])
    };
    let (ed_reg, ed_bold, ed_it) = files
        .map(|[r, b, i]| {
            (
                load("ed", &[&r]).then_some("ed"),
                load("ed-bold", &[&b]).then_some("ed-bold"),
                load("ed-italic", &[&i]).then_some("ed-italic"),
            )
        })
        .unwrap_or((None, None, None));

    let fallback = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let chain = |first: &[Option<&str>]| -> Vec<String> {
        let mut v: Vec<String> = first.iter().flatten().map(|s| (*s).to_owned()).collect();
        if regular {
            v.push("ui".into());
        }
        v.extend(fallback.iter().cloned());
        if symbols {
            v.push("symbols".into());
        }
        v.dedup();
        v
    };
    let ui_bold = semibold.then_some("ui-bold");
    let ui_it = italic.then_some("ui-italic");
    let ui_bi = if semibold_italic { Some("ui-bold-italic") } else { ui_bold };
    fonts.families.insert(FontFamily::Proportional, chain(&[]));
    fonts.families.insert(FontFamily::Name(BOLD.into()), chain(&[ui_bold]));
    fonts.families.insert(FontFamily::Name(ITALIC.into()), chain(&[ui_it]));
    fonts.families.insert(FontFamily::Name(BOLD_ITALIC.into()), chain(&[ui_bi]));
    // Famiglie dell'editor: il font scelto, poi Segoe UI come riserva.
    fonts.families.insert(FontFamily::Name("editor".into()), chain(&[ed_reg]));
    fonts.families.insert(FontFamily::Name("editor-bold".into()), chain(&[ed_bold, ui_bold]));
    fonts.families.insert(FontFamily::Name("editor-italic".into()), chain(&[ed_it, ui_it]));
    fonts.families.insert(FontFamily::Name("editor-bold-italic".into()), chain(&[ed_bold, ui_bi]));
    if mono {
        fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "mono".into());
    }
    ctx.set_fonts(fonts);
}

/// Prefisso dei font importati nelle impostazioni: `file:<nome del file>`.
pub const IMPORTED: &str = "file:";

fn is_valid_font(bytes: &[u8]) -> bool {
    skrifa::FontRef::from_index(bytes, 0).is_ok()
}

/// Cartella dei font importati dall'utente.
pub fn user_fonts_dir() -> Option<std::path::PathBuf> {
    Settings::dir().map(|d| d.join("fonts"))
}

/// `Inter-Regular.ttf` + "Bold" → `Inter-Bold.ttf`, se esiste.
fn sibling_variant(path: &std::path::Path, variant: &str) -> Option<std::path::PathBuf> {
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let ext = path.extension()?.to_string_lossy().into_owned();
    let base =
        ["-Regular", "_Regular", " Regular", "-Book", "Regular"].iter().find_map(|r| stem.strip_suffix(r)).unwrap_or(&stem).to_owned();
    let dir = path.parent()?;
    ["-", "_", " ", ""].iter().map(|sep| dir.join(format!("{base}{sep}{variant}.{ext}"))).find(|p| p.exists())
}

/// Nome da mostrare per un font importato (`Inter-Regular.ttf` → `Inter`).
pub fn font_display_name(file: &str) -> String {
    let stem = std::path::Path::new(file).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    for r in ["-Regular", "_Regular", " Regular"] {
        if let Some(b) = stem.strip_suffix(r) {
            return b.to_owned();
        }
    }
    stem
}

/// Font importati (solo le versioni "normali": le varianti bold/italic vengono usate in automatico).
pub fn imported_fonts() -> Vec<String> {
    let Some(dir) = user_fonts_dir() else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            let p = std::path::Path::new(n);
            let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let stem = p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            // Le varianti (Inter-Bold, Inter-Italic…) non compaiono da sole nell'elenco.
            let variant = ["bold", "italic", "oblique", "light", "medium", "black", "thin"].iter().any(|v| stem.ends_with(v));
            matches!(ext.as_str(), "ttf" | "otf" | "ttc") && !variant
        })
        .collect();
    out.sort();
    out
}

/// Copia i file scelti nella cartella dei font. Importa tutti quelli validi:
/// restituisce il primo importato e gli errori dei file che non è riuscito a copiare.
pub fn import_fonts(files: &[std::path::PathBuf]) -> (Option<String>, Vec<String>) {
    let Some(dir) = user_fonts_dir() else { return (None, vec!["cartella impostazioni non trovata".into()]) };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return (None, vec![e.to_string()]);
    }
    let mut first = None;
    let mut errors = Vec::new();
    for f in files {
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let result = std::fs::read(f).map_err(|e| e.to_string()).and_then(|bytes| {
            if !is_valid_font(&bytes) {
                return Err(crate::i18n::tr("non è un font valido", "is not a valid font").to_owned());
            }
            crate::fsutil::write_atomic(&dir.join(&name), &bytes).map_err(|e| e.to_string())
        });
        match result {
            Ok(()) => {
                // Le varianti (Bold/Italic) si usano in automatico: si sceglie la versione normale.
                let stem = name.to_lowercase();
                let is_variant =
                    ["bold", "italic", "oblique"].iter().any(|v| stem.trim_end_matches(".ttf").trim_end_matches(".otf").ends_with(v));
                if first.is_none()
                    || (!is_variant
                        && first.as_deref().is_some_and(|f: &str| f.to_lowercase().contains("bold") || f.to_lowercase().contains("italic")))
                {
                    first = Some(name);
                }
            }
            Err(e) => errors.push(format!("«{name}»: {e}")),
        }
    }
    (first, errors)
}

/// Famiglia del testo delle note.
pub fn editor_family(bold: bool, italic: bool) -> FontFamily {
    FontFamily::Name(
        match (bold, italic) {
            (false, false) => "editor",
            (true, false) => "editor-bold",
            (false, true) => "editor-italic",
            (true, true) => "editor-bold-italic",
        }
        .into(),
    )
}

// ---------------------------------------------------------------------------
// Colori

/// Tutti i colori dell'interfaccia per un tema (chiaro o scuro).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub dark: bool,
    /// Sfondo della finestra quando lo sfondo trasparente è spento.
    pub ground: Color32,
    pub sidebar: Color32,
    pub list: Color32,
    pub editor: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub border: Color32,
    pub hover: Color32,
    /// Accento così come scelto.
    pub accent: Color32,
    /// Accento "bianco e nero" (segue il colore del testo).
    pub mono: bool,
    /// Accento scurito o schiarito per essere leggibile come testo.
    pub accent_text: Color32,
    /// Riempimento delle righe selezionate.
    pub accent_fill: Color32,
    /// Testo sopra un riempimento pieno d'accento.
    pub on_accent: Color32,
    pub code_bg: Color32,
    pub danger: Color32,
}

fn hex(c: &str) -> Option<Color32> {
    let c = c.trim().trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(c.get(i..i + 2)?, 16).ok();
    match c.len() {
        6 => Some(Color32::from_rgb(p(0)?, p(2)?, p(4)?)),
        8 => Some(Color32::from_rgba_unmultiplied(p(0)?, p(2)?, p(4)?, p(6)?)),
        _ => None,
    }
}

fn luminance(c: Color32) -> f32 {
    let f = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
}

pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Avvicina `c` al bianco o al nero finché il contrasto con `bg` non raggiunge `ratio`.
pub fn readable_on(c: Color32, bg: Color32, ratio: f32) -> Color32 {
    let target = if luminance(bg) > 0.4 { Color32::BLACK } else { Color32::WHITE };
    (0..=20).map(|i| mix(c, target, i as f32 / 20.0)).find(|x| contrast(*x, bg) >= ratio).unwrap_or(target)
}

fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

impl Palette {
    pub fn light(accent: Color32) -> Self {
        Self::finish(Self {
            dark: false,
            ground: Color32::from_rgb(0xF3, 0xF3, 0xF3),
            sidebar: Color32::from_rgb(0xF3, 0xF3, 0xF3),
            list: Color32::from_rgb(0xF9, 0xF9, 0xF9),
            editor: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            text: Color32::from_rgb(0x1B, 0x1B, 0x1B),
            text_muted: Color32::from_rgb(0x5E, 0x5E, 0x5E),
            border: Color32::from_rgba_unmultiplied(0, 0, 0, 0x16),
            hover: Color32::from_rgba_unmultiplied(0, 0, 0, 0x0B),
            mono: accent == Color32::from_rgb(MONO[0], MONO[1], MONO[2]),
            accent,
            accent_text: accent,
            accent_fill: accent,
            on_accent: Color32::WHITE,
            code_bg: Color32::from_rgba_unmultiplied(0, 0, 0, 0x0D),
            danger: Color32::from_rgb(0xC4, 0x2B, 0x1C),
        })
    }

    pub fn dark(accent: Color32) -> Self {
        Self::finish(Self {
            dark: true,
            ground: Color32::from_rgb(0x20, 0x20, 0x20),
            sidebar: Color32::from_rgb(0x1C, 0x1C, 0x1C),
            list: Color32::from_rgb(0x22, 0x22, 0x22),
            editor: Color32::from_rgb(0x28, 0x28, 0x28),
            text: Color32::from_rgb(0xF2, 0xF2, 0xF2),
            text_muted: Color32::from_rgb(0xA6, 0xA6, 0xA6),
            border: Color32::from_rgba_unmultiplied(255, 255, 255, 0x14),
            hover: Color32::from_rgba_unmultiplied(255, 255, 255, 0x0E),
            mono: accent == Color32::from_rgb(MONO[0], MONO[1], MONO[2]),
            accent,
            accent_text: accent,
            accent_fill: accent,
            on_accent: Color32::BLACK,
            code_bg: Color32::from_rgba_unmultiplied(255, 255, 255, 0x10),
            danger: Color32::from_rgb(0xFF, 0x99, 0xA4),
        })
    }

    /// Calcola i colori derivati dall'accento.
    fn finish(mut self) -> Self {
        if self.mono {
            self.accent = Color32::from_rgb(self.text.r(), self.text.g(), self.text.b());
        }
        self.accent_text = readable_on(self.accent, self.editor, 4.5);
        // In bianco e nero la selezione è un grigio leggero, non un blocco pieno.
        let fill = match (self.mono, self.dark) {
            (true, false) => 0.09,
            (true, true) => 0.14,
            (false, false) => 0.22,
            (false, true) => 0.30,
        };
        self.accent_fill = with_alpha(self.accent, fill);
        self.on_accent = if contrast(Color32::WHITE, self.accent) >= contrast(Color32::BLACK, self.accent) * 0.8 {
            Color32::WHITE
        } else {
            Color32::BLACK
        };
        self
    }

    /// Applica un tema da file (solo i colori presenti nel file).
    fn with_file(mut self, file: &ThemeFile) -> Self {
        let c = &file.colors;
        let set = |slot: &mut Color32, v: &Option<String>| {
            if let Some(col) = v.as_deref().and_then(hex) {
                *slot = col;
            }
        };
        set(&mut self.ground, &c.background);
        set(&mut self.editor, &c.background);
        set(&mut self.sidebar, &c.sidebar);
        set(&mut self.list, &c.list);
        set(&mut self.editor, &c.editor);
        set(&mut self.text, &c.text);
        set(&mut self.text_muted, &c.text_muted);
        set(&mut self.border, &c.border);
        set(&mut self.code_bg, &c.code_bg);
        set(&mut self.accent, &c.accent);
        Self::finish(self)
    }
}

// ---------------------------------------------------------------------------
// Temi da file

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct ThemeColors {
    pub background: Option<String>,
    pub sidebar: Option<String>,
    pub list: Option<String>,
    pub editor: Option<String>,
    pub text: Option<String>,
    pub text_muted: Option<String>,
    pub border: Option<String>,
    pub code_bg: Option<String>,
    pub accent: Option<String>,
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct ThemeFile {
    pub name: String,
    /// "light" o "dark": i colori mancanti vengono da questo tema.
    pub base: String,
    pub colors: ThemeColors,
}

impl ThemeFile {
    pub fn is_dark(&self) -> bool {
        self.base.eq_ignore_ascii_case("dark")
    }
}

const BUILTIN_THEMES: [(&str, &str); 2] = [
    (
        "Carta",
        "name = \"Carta\"\nbase = \"light\"\n\n[colors]\nsidebar = \"#F1ECE1\"\nlist = \"#F7F3EA\"\neditor = \"#FCFAF5\"\ntext = \"#2B2620\"\ntext_muted = \"#6E6455\"\nborder = \"#2B262018\"\ncode_bg = \"#2B262010\"\n",
    ),
    (
        "Notte",
        "name = \"Notte\"\nbase = \"dark\"\n\n[colors]\nsidebar = \"#000000\"\nlist = \"#0A0A0A\"\neditor = \"#111111\"\ntext = \"#ECECEC\"\ntext_muted = \"#9A9A9A\"\n",
    ),
];

pub fn themes_dir() -> Option<std::path::PathBuf> {
    Settings::dir().map(|d| d.join("themes"))
}

/// Scrive i temi d'esempio nella cartella dei temi, se mancano.
pub fn ensure_example_themes() {
    let Some(dir) = themes_dir() else { return };
    for (name, body) in BUILTIN_THEMES {
        let path = dir.join(format!("{name}.toml"));
        if !path.exists() {
            let _ = crate::fsutil::write_atomic(&path, body.as_bytes());
        }
    }
}

/// Nomi dei temi disponibili (inclusi e file dell'utente).
pub fn available_themes() -> Vec<String> {
    let mut names: Vec<String> = BUILTIN_THEMES.iter().map(|(n, _)| (*n).to_owned()).collect();
    if let Some(dir) = themes_dir()
        && let Ok(entries) = std::fs::read_dir(dir)
    {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("toml"))
                && let Some(stem) = p.file_stem()
            {
                names.push(stem.to_string_lossy().into_owned());
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

pub fn load_theme(name: &str) -> Option<ThemeFile> {
    if name.is_empty() {
        return None;
    }
    let from_file = themes_dir().and_then(|d| std::fs::read_to_string(d.join(format!("{name}.toml"))).ok());
    let text = from_file.or_else(|| BUILTIN_THEMES.iter().find(|(n, _)| *n == name).map(|(_, b)| (*b).to_owned()))?;
    toml::from_str(text.trim_start_matches('\u{feff}')).ok()
}

/// Data di modifica del file del tema: serve a ricaricarlo quando cambia.
pub fn theme_mtime(name: &str) -> Option<std::time::SystemTime> {
    themes_dir().and_then(|d| std::fs::metadata(d.join(format!("{name}.toml"))).ok()?.modified().ok())
}

// ---------------------------------------------------------------------------
// Applicazione

fn palette_id(dark: bool) -> Id {
    Id::unique(("yonote-palette", dark))
}

/// La palette del tema attivo.
pub fn palette(ctx: &egui::Context) -> Palette {
    let dark = ctx.theme() == egui::Theme::Dark;
    ctx.data(|d| d.get_temp(palette_id(dark)))
        .unwrap_or_else(|| if dark { Palette::dark(Color32::GOLD) } else { Palette::light(Color32::GOLD) })
}

/// Opacità dei pannelli (1.0 = pieni) secondo lo sfondo scelto.
pub fn panel_alpha(settings: &Settings) -> [f32; 3] {
    match settings.backdrop {
        Backdrop::Off => [1.0; 3],
        _ => {
            let o = settings.backdrop_opacity;
            [o, (o + 0.12).min(1.0), (o + 0.22).clamp(0.9, 1.0)]
        }
    }
}

/// Colori effettivi dei tre pannelli, con la trasparenza applicata.
pub fn panel_fills(ctx: &egui::Context, settings: &Settings) -> [Color32; 3] {
    let p = palette(ctx);
    let a = panel_alpha(settings);
    let f = |c: Color32, a: f32| {
        let base = c.a() as f32 / 255.0;
        with_alpha(Color32::from_rgb(c.r(), c.g(), c.b()), base * a)
    };
    [f(p.sidebar, a[0]), f(p.list, a[1]), f(p.editor, a[2])]
}

fn visuals_for(p: &Palette) -> egui::Visuals {
    let mut v = if p.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    let solid_editor = Color32::from_rgb(p.editor.r(), p.editor.g(), p.editor.b());
    v.panel_fill = solid_editor;
    v.window_fill = solid_editor;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = 12.into();
    v.window_shadow = Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(if p.dark { 110 } else { 40 }) };
    v.popup_shadow = Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(if p.dark { 90 } else { 30 }) };
    v.menu_corner_radius = 8.into();
    v.extreme_bg_color =
        if p.dark { Color32::from_rgba_unmultiplied(255, 255, 255, 0x0F) } else { Color32::from_rgba_unmultiplied(0, 0, 0, 0x08) };
    v.text_edit_bg_color = Some(v.extreme_bg_color);
    v.faint_bg_color = p.hover;
    v.code_bg_color = p.code_bg;
    v.hyperlink_color = p.accent_text;
    v.weak_text_color = Some(p.text_muted);
    v.selection.bg_fill = with_alpha(p.accent, if p.dark { 0.42 } else { 0.32 });
    v.selection.stroke = Stroke::new(1.5, p.accent_text);
    v.text_cursor.stroke = Stroke::new(2.0, p.accent_text);
    v.error_fg_color = p.danger;

    for (w, fg) in [
        (&mut v.widgets.noninteractive, p.text),
        (&mut v.widgets.inactive, p.text),
        (&mut v.widgets.hovered, p.text),
        (&mut v.widgets.active, p.text),
        (&mut v.widgets.open, p.text),
    ] {
        w.fg_stroke = Stroke::new(1.0, fg);
        w.corner_radius = 6.into();
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.noninteractive.bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_fill = p.hover;
    v.widgets.inactive.weak_bg_fill = p.hover;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.bg_fill = p.hover.gamma_multiply(1.8);
    v.widgets.hovered.weak_bg_fill = p.hover.gamma_multiply(1.8);
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.active.bg_fill = p.hover.gamma_multiply(2.6);
    v.widgets.active.weak_bg_fill = p.hover.gamma_multiply(2.6);
    v.widgets.active.bg_stroke = Stroke::NONE;
    v.widgets.open.weak_bg_fill = p.hover.gamma_multiply(1.8);
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;
    v.striped = false;
    v
}

/// Ricalcola palette e stili da impostazioni e tema da file.
pub fn apply(ctx: &egui::Context, settings: &Settings) {
    let [r, g, b] = settings.accent;
    let accent = Color32::from_rgb(r, g, b);
    let mut light = Palette::light(accent);
    let mut dark = Palette::dark(accent);
    if let Some(t) = load_theme(&settings.custom_theme) {
        if t.is_dark() {
            dark = dark.with_file(&t);
        } else {
            light = light.with_file(&t);
        }
    }
    ctx.data_mut(|d| {
        d.insert_temp(palette_id(false), light);
        d.insert_temp(palette_id(true), dark);
    });
    ctx.set_visuals_of(Theme::Light, visuals_for(&light));
    ctx.set_visuals_of(Theme::Dark, visuals_for(&dark));
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(10.0, 5.0);
        s.spacing.interact_size.y = 28.0;
        s.spacing.slider_width = 180.0;
        s.spacing.scroll = egui::style::ScrollStyle::floating();
        s.interaction.selectable_labels = false;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_text_is_always_readable() {
        for (_, _, [r, g, b]) in ACCENTS {
            let a = Color32::from_rgb(r, g, b);
            for p in [Palette::light(a), Palette::dark(a)] {
                assert!(contrast(p.accent_text, p.editor) >= 4.5, "{a:?}");
            }
        }
    }

    #[test]
    fn base_text_contrast_meets_aa() {
        for p in [Palette::light(Color32::RED), Palette::dark(Color32::RED)] {
            for bg in [p.sidebar, p.list, p.editor] {
                assert!(contrast(p.text, bg) >= 7.0);
                assert!(contrast(p.text_muted, bg) >= 4.5, "muted {:?} on {:?}", p.text_muted, bg);
            }
        }
    }

    #[test]
    fn font_names_and_variants() {
        assert_eq!(font_display_name("Inter-Regular.ttf"), "Inter");
        assert_eq!(font_display_name("Lora.ttf"), "Lora");
        let dir = std::env::temp_dir().join(format!("yonote-fonts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Inter-Regular.ttf"), b"x").unwrap();
        std::fs::write(dir.join("Inter-Bold.ttf"), b"x").unwrap();
        assert_eq!(sibling_variant(&dir.join("Inter-Regular.ttf"), "Bold"), Some(dir.join("Inter-Bold.ttf")));
        assert_eq!(sibling_variant(&dir.join("Inter-Regular.ttf"), "Italic"), None);
        assert!(!is_valid_font(b"x"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn builtin_themes_parse() {
        for (name, body) in BUILTIN_THEMES {
            let t: ThemeFile = toml::from_str(body).unwrap();
            assert_eq!(t.name, name);
            let p = if t.is_dark() { Palette::dark(Color32::RED) } else { Palette::light(Color32::RED) }.with_file(&t);
            assert!(contrast(p.text_muted, p.editor) >= 4.5);
        }
        assert_eq!(hex("#FF000080"), Some(Color32::from_rgba_unmultiplied(255, 0, 0, 128)));
        assert_eq!(hex("zz"), None);
    }
}
