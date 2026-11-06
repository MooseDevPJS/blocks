//! Finestre: palette Ctrl+K, impostazioni, nuova cartella, conferma eliminazione.

use super::panels::paint_app_mark;
use super::*;

impl BlocksApp {
    pub(super) fn palette_window(&mut self, ctx: &egui::Context) {
        if !self.palette.open {
            return;
        }
        let p = theme::palette(ctx);
        // Voci: note (ricerca fuzzy) o comandi se la ricerca inizia con ">".
        let q = self.palette.query.trim().to_owned();
        let mut items: Vec<PaletteItem> = Vec::new();
        if let Some(cq) = q.strip_prefix('>') {
            let labels: Vec<String> = Command::ALL.iter().map(|c| c.label().to_owned()).collect();
            for l in vault::fuzzy_rank(cq.trim(), &labels, 20) {
                if let Some(c) = Command::ALL.iter().find(|c| c.label() == l) {
                    items.push(PaletteItem::Command(*c));
                }
            }
        } else if let Some(v) = &self.vault {
            let mut notes: Vec<&NoteMeta> = v.notes.iter().collect();
            notes.sort_by_key(|n| std::cmp::Reverse(n.modified));
            let titles: Vec<String> = notes.iter().map(|n| n.title.clone()).collect();
            for t in vault::fuzzy_rank(&q, &titles, 12) {
                if let Some(n) = notes.iter().find(|n| n.title == t) {
                    items.push(PaletteItem::Note(n.path.clone(), n.title.clone()));
                }
            }
            if !q.is_empty() {
                if !titles.iter().any(|t| t.eq_ignore_ascii_case(&q)) {
                    items.push(PaletteItem::Create(q.clone()));
                }
                let labels: Vec<String> = Command::ALL.iter().map(|c| c.label().to_owned()).collect();
                for l in vault::fuzzy_rank(&q, &labels, 3) {
                    if let Some(c) = Command::ALL.iter().find(|c| c.label() == l) {
                        items.push(PaletteItem::Command(*c));
                    }
                }
            }
        }
        let n = items.len();
        if n > 0 {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                self.palette.selected = (self.palette.selected + 1) % n;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                self.palette.selected = (self.palette.selected + n - 1) % n;
            }
        }
        self.palette.selected = self.palette.selected.min(n.saturating_sub(1));
        let mut chosen: Option<PaletteItem> = None;
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            chosen = items.get(self.palette.selected).cloned();
        }
        let mut close = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));

        let screen = ctx.content_rect();
        let width = 560.0_f32.min(screen.width() - 40.0);
        // Sfondo scuro dietro la palette: un clic fuori la chiude.
        egui::Area::new(Id::unique("palette-scrim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
            let resp = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(if p.dark { 110 } else { 50 }));
            if resp.clicked() {
                close = true;
            }
        });
        egui::Area::new(Id::unique("palette"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(screen.center().x - width / 2.0, screen.min.y + screen.height() * 0.14))
            .show(ctx, |ui| {
                egui::Frame::window(ui.style()).inner_margin(8).corner_radius(14).show(ui, |ui| {
                    ui.set_width(width);
                    let id = Id::unique("palette-input");
                    let resp = widgets::search_field(
                        ui,
                        &mut self.palette.query,
                        tr("Cerca una nota, oppure > per i comandi", "Search a note, or > for commands"),
                        id,
                    );
                    if self.palette.just_opened {
                        resp.request_focus();
                        self.palette.just_opened = false;
                    }
                    if resp.changed() {
                        self.palette.selected = 0;
                    }
                    ui.add_space(6.0);
                    if items.is_empty() {
                        ui.add_space(10.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new(tr("Nessuna nota trovata", "No notes found")).color(p.text_muted))
                        });
                        ui.add_space(10.0);
                    }
                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        for (i, item) in items.iter().enumerate() {
                            let (icon, label, hint) = match item {
                                PaletteItem::Note(_, t) => (Icon::Notes, t.clone(), String::new()),
                                PaletteItem::Create(t) => {
                                    (Icon::Compose, format!("{} «{t}»", tr("Crea la nota", "Create note")), String::new())
                                }
                                PaletteItem::Command(c) => (c.icon(), c.label().to_owned(), c.shortcut().to_owned()),
                            };
                            let (r, row) = ui.allocate_exact_size(Vec2::new(width, 36.0), Sense::click());
                            if i == self.palette.selected {
                                ui.painter().rect_filled(r, 8.0, p.accent_fill);
                                ui.scroll_to_rect(r, None);
                            } else if row.hovered() {
                                ui.painter().rect_filled(r, 8.0, p.hover);
                            }
                            icons::paint(
                                ui,
                                icon,
                                Rect::from_center_size(Pos2::new(r.min.x + 18.0, r.center().y), Vec2::splat(17.0)),
                                p.text_muted,
                            );
                            let text_clip = Rect::from_min_max(Pos2::new(r.min.x + 34.0, r.min.y), Pos2::new(r.max.x - 12.0, r.max.y));
                            ui.painter().with_clip_rect(text_clip).text(
                                Pos2::new(r.min.x + 38.0, r.center().y),
                                Align2::LEFT_CENTER,
                                label,
                                FontId::proportional(14.0),
                                p.text,
                            );
                            if !hint.is_empty() {
                                ui.painter().text(
                                    r.right_center() - Vec2::new(12.0, 0.0),
                                    Align2::RIGHT_CENTER,
                                    hint,
                                    FontId::proportional(12.0),
                                    p.text_muted,
                                );
                            }
                            if row.clicked() {
                                chosen = Some(item.clone());
                            }
                        }
                    });
                    ui.add_space(4.0);
                    ui.separator();
                    ui.label(
                        RichText::new(tr(
                            "↑↓ per scegliere  ·  Invio per aprire  ·  Esc per chiudere",
                            "↑↓ to choose  ·  Enter to open  ·  Esc to close",
                        ))
                        .size(11.5)
                        .color(p.text_muted),
                    );
                });
            });

        if let Some(item) = chosen {
            self.palette.open = false;
            match item {
                PaletteItem::Note(path, _) => {
                    self.open_note(&path);
                    self.editor.request_focus();
                }
                PaletteItem::Create(t) => {
                    self.open_or_create_by_title(&t);
                    self.editor.request_focus();
                }
                PaletteItem::Command(c) => self.run_command(ctx, c),
            }
        } else if close {
            self.palette.open = false;
        }
    }

    pub(super) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let mut change_vault = false;
        let mut import_font = false;
        let mut toggle_open_with = false;
        let open_with = self.open_with_registered;
        let p = theme::palette(ctx);
        let s = &mut self.settings;
        let close = widgets::dialog(ctx, "settings", tr("Impostazioni", "Settings"), 540.0, |ui| {
            let h = (ctx.content_rect().height() - 170.0).max(200.0);
            egui::ScrollArea::vertical().max_height(h).min_scrolled_height(h.min(640.0)).show(ui, |ui| {
                // Spazio a destra per la barra di scorrimento, così non copre i controlli.
                egui::Frame::NONE.inner_margin(egui::Margin { right: 22, ..Default::default() }).show(ui, |ui| {
                    widgets::settings_heading(ui, tr("Aspetto", "Appearance"));
                    widgets::setting_row(ui, tr("Tema", "Theme"), "", |ui| {
                        widgets::segmented(
                            ui,
                            &mut s.theme,
                            &[
                                (ThemeMode::System, tr("Sistema", "System")),
                                (ThemeMode::Light, tr("Chiaro", "Light")),
                                (ThemeMode::Dark, tr("Scuro", "Dark")),
                            ],
                        );
                    });
                    widgets::setting_row(
                        ui,
                        tr("Sfondo", "Background"),
                        tr("Mica e Acrylic lasciano trasparire il desktop.", "Mica and Acrylic let the desktop show through."),
                        |ui| {
                            widgets::segmented(
                                ui,
                                &mut s.backdrop,
                                &[(Backdrop::Mica, "Mica"), (Backdrop::Acrylic, "Acrylic"), (Backdrop::Off, tr("Pieno", "Solid"))],
                            );
                        },
                    );
                    if s.backdrop != Backdrop::Off {
                        widgets::setting_row(
                            ui,
                            tr("Opacità dei pannelli", "Panel opacity"),
                            tr("Più alta = testo più leggibile.", "Higher = easier to read."),
                            |ui| {
                                ui.add(egui::Slider::new(&mut s.backdrop_opacity, 0.3..=1.0).show_value(false));
                            },
                        );
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new(tr("Colore d'accento", "Accent color")).size(14.0).color(p.text));
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for (name, rgb) in theme::ACCENTS {
                            let c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                            if widgets::swatch(ui, c, s.accent == rgb, name).clicked() {
                                s.accent = rgb;
                            }
                        }
                        ui.add_space(8.0);
                        let custom = !theme::ACCENTS.iter().any(|(_, a)| *a == s.accent);
                        ui.label(RichText::new(tr("Personalizzato", "Custom")).size(13.0).color(if custom {
                            p.text
                        } else {
                            p.text_muted
                        }));
                        ui.color_edit_button_srgb(&mut s.accent).on_hover_text(tr("Scegli qualsiasi colore", "Pick any color"));
                    });
                    ui.add_space(6.0);
                    let themes = theme::available_themes();
                    widgets::setting_row(
                        ui,
                        tr("Tema da file", "Theme file"),
                        tr("File .toml nella cartella dei temi.", ".toml files in the themes folder."),
                        |ui| {
                            if ui.button(tr("Apri cartella", "Open folder")).clicked()
                                && let Some(d) = theme::themes_dir()
                            {
                                crate::platform::open_folder(&d);
                            }
                            let current = if s.custom_theme.is_empty() { tr("Nessuno", "None").to_owned() } else { s.custom_theme.clone() };
                            egui::ComboBox::from_id_salt("custom-theme").selected_text(current).show_ui(ui, |ui| {
                                ui.selectable_value(&mut s.custom_theme, String::new(), tr("Nessuno", "None"));
                                for t in &themes {
                                    if ui.selectable_value(&mut s.custom_theme, t.clone(), t).clicked()
                                        && let Some(f) = theme::load_theme(t)
                                    {
                                        s.theme = if f.is_dark() { ThemeMode::Dark } else { ThemeMode::Light };
                                    }
                                }
                            });
                        },
                    );

                    ui.add_space(8.0);
                    ui.separator();
                    widgets::settings_heading(ui, tr("Scrittura", "Writing"));
                    let imported = theme::imported_fonts();
                    widgets::setting_row(
                        ui,
                        tr("Font del testo", "Text font"),
                        tr("Puoi importare file .ttf e .otf.", "You can import .ttf and .otf files."),
                        |ui| {
                            if ui
                                .button(tr("Importa…", "Import…"))
                                .on_hover_text(tr("Aggiungi un font dal PC", "Add a font from your PC"))
                                .clicked()
                            {
                                import_font = true;
                            }
                            let current = match s.editor_font.strip_prefix(theme::IMPORTED) {
                                Some(f) => theme::font_display_name(f),
                                None if s.editor_font.is_empty() => "Segoe UI".to_owned(),
                                None => s.editor_font.clone(),
                            };
                            egui::ComboBox::from_id_salt("editor-font").width(170.0).height(420.0).selected_text(current).show_ui(
                                ui,
                                |ui| {
                                    for (name, ..) in theme::EDITOR_FONTS {
                                        let value = if name == "Segoe UI" { String::new() } else { name.to_owned() };
                                        ui.selectable_value(&mut s.editor_font, value, name);
                                    }
                                    if !imported.is_empty() {
                                        ui.separator();
                                        ui.label(RichText::new(tr("Importati", "Imported")).size(12.0).color(p.text_muted));
                                        for f in &imported {
                                            ui.selectable_value(
                                                &mut s.editor_font,
                                                format!("{}{f}", theme::IMPORTED),
                                                theme::font_display_name(f),
                                            );
                                        }
                                    }
                                    ui.separator();
                                    if ui.button(tr("Apri la cartella dei font", "Open the fonts folder")).clicked()
                                        && let Some(d) = theme::user_fonts_dir()
                                    {
                                        let _ = std::fs::create_dir_all(&d);
                                        crate::platform::open_folder(&d);
                                    }
                                },
                            );
                        },
                    );
                    widgets::setting_row(ui, tr("Dimensione", "Size"), "Ctrl + / Ctrl −", |ui| {
                        ui.add(egui::Slider::new(&mut s.font_size, 10.0..=32.0).step_by(1.0).suffix(" pt"));
                    });
                    widgets::setting_row(ui, tr("Interlinea", "Line spacing"), "", |ui| {
                        ui.add(egui::Slider::new(&mut s.line_spacing, 1.1..=2.2).step_by(0.05).fixed_decimals(2));
                    });
                    widgets::setting_row(ui, tr("Larghezza del testo", "Text width"), "", |ui| {
                        ui.add(egui::Slider::new(&mut s.text_width, 480.0..=1400.0).step_by(20.0).suffix(" px"));
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    widgets::settings_heading(ui, tr("Note", "Notes"));
                    let root = s.vault.clone().map(|v| v.display().to_string()).unwrap_or_default();
                    widgets::setting_row(ui, tr("Cartella", "Folder"), &widgets::middle_ellipsis(&root, 44), |ui| {
                        if ui.button(tr("Cambia…", "Change…")).on_hover_text(&root).clicked() {
                            change_vault = true;
                        }
                    });
                    widgets::setting_row(ui, tr("Lingua", "Language"), tr("Si applica subito.", "Applies immediately."), |ui| {
                        widgets::segmented(
                            ui,
                            &mut s.language,
                            &[(Language::Auto, "Auto"), (Language::It, "Italiano"), (Language::En, "English")],
                        );
                    });
                    let help = if open_with {
                        tr(
                            "Attivo: i file .md si aprono e si modificano direttamente in Blocks.",
                            "On: .md files open and are edited directly in Blocks.",
                        )
                    } else {
                        tr("Tasto destro su un file .md → Apri con → Blocks.", "Right-click a .md file → Open with → Blocks.")
                    };
                    widgets::setting_row(ui, tr("«Apri con» per i file Markdown", "\"Open with\" for Markdown files"), help, |ui| {
                        let label = if open_with { tr("Rimuovi", "Remove") } else { tr("Aggiungi", "Add") };
                        if ui.button(label).clicked() {
                            toggle_open_with = true;
                        }
                    });

                    ui.add_space(14.0);
                    if widgets::kofi_card(ui) {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(KOFI_URL));
                    }
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                        paint_app_mark(ui, r, p.accent);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(format!("Blocks {}", env!("CARGO_PKG_VERSION"))).size(14.0).strong().color(p.text));
                            ui.label(
                                RichText::new(tr(
                                    "Gratis per uso personale. Codice aperto, nessuna telemetria.",
                                    "Free for personal use. Source available, no telemetry.",
                                ))
                                .size(12.5)
                                .color(p.text_muted),
                            );
                        });
                    });
                    ui.add_space(4.0);
                });
            });
        });
        if close {
            self.show_settings = false;
        }
        if change_vault {
            self.choose_vault(ctx);
        }
        if import_font
            && let Some(files) = rfd::FileDialog::new()
                .set_title(tr("Scegli uno o più font", "Choose one or more fonts"))
                .add_filter("Font", &["ttf", "otf", "ttc"])
                .pick_files()
        {
            let (first, errors) = theme::import_fonts(&files);
            if let Some(name) = first {
                self.settings.editor_font = format!("{}{name}", theme::IMPORTED);
            }
            if !errors.is_empty() {
                self.error =
                    Some(format!("{}: {}", tr("Alcuni font non sono stati importati", "Some fonts were not imported"), errors.join(", ")));
            }
        }
        if toggle_open_with {
            let result = if open_with { crate::platform::unregister_open_with() } else { crate::platform::register_open_with() };
            match result {
                Ok(()) => self.open_with_registered = crate::platform::is_registered(),
                Err(e) => self.error = Some(format!("{}: {e}", tr("Impossibile modificare «Apri con»", "Could not change \"Open with\""))),
            }
        }
    }

    pub(super) fn new_folder_window(&mut self, ctx: &egui::Context) {
        let Some((parent, name)) = &mut self.new_folder else { return };
        let mut done = false;
        let mut cancel = false;
        let closed = widgets::dialog(ctx, "new-folder", tr("Nuova cartella", "New folder"), 340.0, |ui| {
            let r = ui.add(
                egui::TextEdit::singleline(name)
                    .hint_text(tr("Nome della cartella", "Folder name"))
                    .desired_width(f32::INFINITY)
                    .margin(Vec2::new(10.0, 7.0)),
            );
            r.request_focus();
            if ui.input(|i| i.key_pressed(Key::Enter)) {
                done = true;
            }
            ui.add_space(14.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button(ui, tr("Crea", "Create"), true).clicked() {
                    done = true;
                }
                if widgets::button(ui, tr("Annulla", "Cancel"), false).clicked() {
                    cancel = true;
                }
            });
        });
        cancel |= closed;
        if done && !name.trim().is_empty() {
            let dir = parent.join(vault::file_stem_for_title(name));
            match std::fs::create_dir_all(&dir) {
                Ok(()) => {
                    if let Some(v) = &mut self.vault {
                        v.rescan();
                    }
                    self.filter = Filter::Folder(dir);
                }
                Err(e) => self.error = Some(format!("{}: {e}", tr("Impossibile creare la cartella", "Could not create the folder"))),
            }
            self.new_folder = None;
        } else if cancel {
            self.new_folder = None;
        }
    }

    /// Finestra di conferma per l'eliminazione.
    pub(super) fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.confirm else { return };
        let p = theme::palette(ctx);
        let (title, body) = match pending {
            PendingDelete::Notes(paths) if paths.len() == 1 => (
                tr("Spostare la nota nel Cestino?", "Move the note to the Recycle Bin?").to_owned(),
                format!(
                    "«{}» {}",
                    self.note_title(&paths[0]),
                    tr("andrà nel Cestino di Windows. Potrai ripristinarla.", "will go to the Windows Recycle Bin. You can restore it.")
                ),
            ),
            PendingDelete::Notes(paths) => (
                format!("{} {} {}", tr("Spostare", "Move"), paths.len(), tr("note nel Cestino?", "notes to the Recycle Bin?")),
                tr(
                    "Andranno nel Cestino di Windows. Potrai ripristinarle.",
                    "They will go to the Windows Recycle Bin. You can restore them.",
                )
                .to_owned(),
            ),
            PendingDelete::Folder(f) => (
                tr("Spostare la cartella nel Cestino?", "Move the folder to the Recycle Bin?").to_owned(),
                format!(
                    "«{}» {}",
                    f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    tr(
                        "e tutte le note al suo interno andranno nel Cestino di Windows.",
                        "and all its notes will go to the Windows Recycle Bin."
                    )
                ),
            ),
        };
        let mut confirmed = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        let mut cancel = false;
        let modal = widgets::dialog(ctx, "confirm-delete", &title, 400.0, |ui| {
            ui.label(RichText::new(&body).size(14.0).color(p.text));
            ui.add_space(18.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::danger_button(ui, tr("Sposta nel Cestino", "Move to Recycle Bin")).clicked() {
                    confirmed = true;
                }
                if widgets::button(ui, tr("Annulla", "Cancel"), false).clicked() {
                    cancel = true;
                }
            });
        });
        if confirmed {
            match self.confirm.take() {
                Some(PendingDelete::Notes(paths)) => self.delete_notes(&paths),
                Some(PendingDelete::Folder(f)) => self.delete_folder(&f),
                None => {}
            }
        } else if cancel || modal {
            self.confirm = None;
        }
    }
}
