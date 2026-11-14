//! Le tre colonne: cartelle, elenco note, editor; più banner e messaggi.

use super::*;

impl YonoteApp {
    pub(super) fn welcome(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(theme::panel_fills(ui.ctx(), &self.settings)[2])).show(ui, |ui| {
            let ctx = ui.ctx().clone();
            ui.vertical_centered(|ui| {
                ui.add_space((ui.available_height() * 0.22).max(24.0));
                let (r, _) = ui.allocate_exact_size(Vec2::splat(72.0), Sense::hover());
                paint_app_mark(ui, r);
                ui.add_space(18.0);
                ui.label(RichText::new("Yonote").size(34.0).family(theme::family(true, false)).color(p.text));
                ui.add_space(4.0);
                ui.label(
                    RichText::new(tr("Note leggere. File tuoi. Niente distrazioni.", "Light notes. Your files. No distractions."))
                        .size(16.0)
                        .color(p.text_muted),
                );
                ui.add_space(34.0);
                ui.label(RichText::new(tr("Dove vuoi tenere le note?", "Where should your notes live?")).size(14.0).color(p.text));
                ui.add_space(12.0);
                let default = Self::default_vault();
                if widgets::button(ui, &format!("{} {}", tr("Usa", "Use"), default.display()), true).clicked() {
                    self.open_vault(&ctx, default);
                }
                ui.add_space(8.0);
                if widgets::button(ui, tr("Scegli un'altra cartella…", "Choose another folder…"), false).clicked() {
                    self.choose_vault(&ctx);
                }
                ui.add_space(16.0);
                ui.label(
                    RichText::new(tr(
                        "Ogni nota è un file Markdown. Puoi usare anche una cartella di Obsidian.",
                        "Every note is a Markdown file. An Obsidian folder works too.",
                    ))
                    .size(12.5)
                    .color(p.text_muted),
                );
            });
        });
    }

    pub(super) fn sidebar(&mut self, ui: &mut egui::Ui) {
        let Some(v) = &self.vault else { return };
        let total = v.notes.len();
        let tree = v.tree.clone();
        let tags = v.tags.clone();
        let p = pal(ui);

        ui.add_space(10.0);
        // Pulsante "cerca o esegui un comando": apre la palette.
        let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
        ui.painter().rect_filled(r, 8.0, if resp.hovered() { p.hover.gamma_multiply(2.0) } else { p.hover });
        icons::paint(ui, Icon::Search, Rect::from_center_size(Pos2::new(r.min.x + 17.0, r.center().y), Vec2::splat(15.0)), p.text_muted);
        ui.painter().text(
            Pos2::new(r.min.x + 32.0, r.center().y),
            Align2::LEFT_CENTER,
            tr("Vai a…", "Go to…"),
            FontId::proportional(13.5),
            p.text_muted,
        );
        ui.painter().text(
            Pos2::new(r.max.x - 10.0, r.center().y),
            Align2::RIGHT_CENTER,
            "Ctrl K",
            FontId::proportional(11.5),
            p.text_muted,
        );
        widgets::a11y(&resp, egui::Role::Button, tr("Vai a una nota o a un comando (Ctrl+K)", "Go to a note or command (Ctrl+K)"), None);
        if resp.on_hover_text(tr("Cerca note ed esegui comandi", "Search notes and run commands")).clicked() {
            self.palette = Palette { open: true, just_opened: true, ..Default::default() };
        }
        ui.add_space(10.0);

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if widgets::nav_row(ui, Icon::Notes, tr("Tutte le note", "All notes"), Some(total), self.filter == Filter::All, 0).clicked() {
                self.filter = Filter::All;
            }
            ui.add_space(8.0);
            let header = widgets::group_label(ui, tr("Cartelle", "Folders"));
            let add = Rect::from_center_size(Pos2::new(header.max.x - 16.0, header.max.y - 12.0), Vec2::splat(24.0));
            let add_resp = ui.interact(add, Id::unique("add-folder"), Sense::click());
            widgets::a11y(&add_resp, egui::Role::Button, tr("Nuova cartella", "New folder"), None);
            if add_resp.hovered() {
                ui.painter().rect_filled(add, 6.0, p.hover);
            }
            icons::paint(ui, Icon::FolderPlus, add.shrink(4.0), if add_resp.hovered() { p.text } else { p.text_muted });
            if add_resp.on_hover_text(tr("Nuova cartella", "New folder")).clicked() {
                self.new_folder = Some((tree.path.clone(), String::new()));
            }
            if tree.children.is_empty() {
                ui.label(RichText::new(tr("   Nessuna cartella", "   No folders")).size(12.5).color(p.text_muted));
            }
            for child in &tree.children {
                self.folder_rows(ui, child, 0);
            }
            if !tags.is_empty() {
                ui.add_space(8.0);
                widgets::group_label(ui, tr("Tag", "Tags"));
                for (t, c) in &tags {
                    let selected = self.filter == Filter::Tag(t.clone());
                    if widgets::nav_row(ui, Icon::Hash, t, Some(*c), selected, 0).clicked() {
                        self.filter = if selected { Filter::All } else { Filter::Tag(t.clone()) };
                    }
                }
            }
            ui.add_space(56.0);
        });
    }

    pub(super) fn folder_rows(&mut self, ui: &mut egui::Ui, folder: &vault::Folder, depth: usize) {
        let Some(v) = &self.vault else { return };
        let count = v.notes.iter().filter(|n| n.path.parent().is_some_and(|p| vault::same_path(p, &folder.path))).count();
        let selected = self.filter == Filter::Folder(folder.path.clone());
        let icon = if selected { Icon::FolderOpen } else { Icon::Folder };
        let resp = widgets::nav_row(ui, icon, &folder.name, Some(count), selected, depth);
        if resp.clicked() {
            self.filter = Filter::Folder(folder.path.clone());
        }
        resp.context_menu(|ui| {
            if ui.button(tr("Nuova nota qui", "New note here")).clicked() {
                self.filter = Filter::Folder(folder.path.clone());
                self.new_note();
            }
            if ui.button(tr("Nuova sottocartella", "New subfolder")).clicked() {
                self.new_folder = Some((folder.path.clone(), String::new()));
            }
            if ui.button(tr("Mostra in Esplora file", "Show in File Explorer")).clicked() {
                crate::platform::open_folder(&folder.path);
            }
            ui.separator();
            if ui.button(RichText::new(tr("Sposta la cartella nel Cestino", "Move folder to Recycle Bin")).color(pal(ui).danger)).clicked()
            {
                self.confirm = Some(PendingDelete::Folder(folder.path.clone()));
            }
        });
        for child in &folder.children {
            self.folder_rows(ui, child, depth + 1);
        }
    }

    pub(super) fn note_list(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(self.filter_title()).size(20.0).family(theme::family(true, false)).color(p.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icons::button(ui, Icon::Compose, tr("Nuova nota (Ctrl+N)", "New note (Ctrl+N)")).clicked() {
                    self.new_note();
                }
            });
        });
        ui.add_space(8.0);
        widgets::search_field(ui, &mut self.search, tr("Cerca", "Search"), Id::unique("yonote-search"));
        ui.add_space(8.0);

        let open_path = self.open_path().map(Path::to_path_buf);
        let new_note_open = self.open.as_ref().is_some_and(|o| o.path.is_none());
        let pinned_set: std::collections::HashSet<String> = self.state.pinned.iter().map(|p| p.to_lowercase()).collect();
        let root = self.vault.as_ref().map(|v| v.root.clone()).unwrap_or_default();
        let (pinned, others): (Vec<&NoteMeta>, Vec<&NoteMeta>) =
            self.sorted_notes().into_iter().partition(|n| pinned_set.contains(&rel(&root, &n.path).to_lowercase()));
        let folders = self.vault.as_ref().map(|v| flatten_folders(&v.tree)).unwrap_or_default();
        let mut clicked: Option<(PathBuf, Modifiers)> = None;
        let mut delete: Option<PathBuf> = None;
        let order: Vec<PathBuf> = pinned.iter().chain(others.iter()).map(|n| n.path.clone()).collect();
        let mut pin: Option<PathBuf> = None;
        let mut moving: Option<(PathBuf, PathBuf)> = None;

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if new_note_open {
                let title = self.open.as_ref().map(|o| vault::title_of(&o.text)).filter(|t| !t.is_empty());
                let title = title.as_deref().unwrap_or(tr("Nuova nota", "New note"));
                let row = widgets::NoteRow {
                    title,
                    date: tr("adesso", "now"),
                    preview: "",
                    thumb: None,
                    pinned: false,
                    selected: true,
                    marked: false,
                };
                widgets::note_row(ui, &row);
            }
            if !pinned.is_empty() {
                list_group(ui, tr("Fissate", "Pinned"));
            }
            let mut last_group = String::new();
            let count = pinned.len() + others.len();
            for (i, n) in pinned.iter().chain(others.iter()).enumerate() {
                let is_pinned = i < pinned.len();
                if !is_pinned {
                    let g = date_group(n.modified);
                    if g != last_group {
                        list_group(ui, &g);
                        last_group = g;
                    }
                }
                let selected = open_path.as_deref().is_some_and(|op| vault::same_path(op, &n.path));
                let marked = self.selection.iter().any(|s| vault::same_path(s, &n.path));
                let preview = if n.preview.is_empty() { tr("Nessun altro testo", "No additional text") } else { &n.preview };
                // Con una selezione multipla attiva conta solo quella: la nota aperta
                // appare piena solo se fa parte della selezione.
                let open_look = selected && (self.selection.is_empty() || marked);
                let date = short_date(n.modified);
                let row = widgets::NoteRow {
                    title: &n.title,
                    date: &date,
                    preview,
                    thumb: n.image.as_deref(),
                    pinned: is_pinned,
                    selected: open_look,
                    marked: marked && !open_look,
                };
                let resp = widgets::note_row(ui, &row);
                if resp.clicked() {
                    clicked = Some((n.path.clone(), ui.input(|i| i.modifiers)));
                }
                resp.context_menu(|ui| {
                    let label = if is_pinned { tr("Sblocca", "Unpin") } else { tr("Fissa in alto", "Pin to top") };
                    if ui.button(label).clicked() {
                        pin = Some(n.path.clone());
                    }
                    ui.menu_button(tr("Sposta in…", "Move to…"), |ui| {
                        for (f, name, depth) in &folders {
                            let here = n.path.parent().is_some_and(|pp| vault::same_path(pp, f));
                            let label = format!("{}{}", "    ".repeat(*depth), name);
                            if ui.add_enabled(!here, egui::Button::new(label)).clicked() {
                                moving = Some((n.path.clone(), f.clone()));
                            }
                        }
                    });
                    if ui.button(tr("Mostra in Esplora file", "Show in File Explorer")).clicked() {
                        crate::platform::reveal_file(&n.path);
                    }
                    ui.separator();
                    let many = marked && self.selection.len() > 1;
                    let label = if many {
                        format!("{} {} {}", tr("Sposta", "Move"), self.selection.len(), tr("note nel Cestino", "notes to Recycle Bin"))
                    } else {
                        tr("Sposta nel Cestino", "Move to Recycle Bin").to_owned()
                    };
                    if ui.button(RichText::new(label).color(pal(ui).danger)).clicked() {
                        delete = Some(n.path.clone());
                    }
                });
                if i + 1 < count && !selected && !marked {
                    widgets::row_divider(ui);
                } else {
                    ui.add_space(1.0);
                }
            }
            if count == 0 && !new_note_open {
                ui.add_space(48.0);
                ui.vertical_centered(|ui| {
                    let icon = if self.search.is_empty() { Icon::Notes } else { Icon::Search };
                    icons::show(ui, icon, 28.0, p.text_muted);
                    ui.add_space(8.0);
                    let msg =
                        if self.search.is_empty() { tr("Nessuna nota qui", "No notes here") } else { tr("Nessun risultato", "No results") };
                    ui.label(RichText::new(msg).size(14.0).color(p.text));
                    ui.add_space(2.0);
                    let hint = if self.search.is_empty() {
                        tr("Premi Ctrl+N per scriverne una.", "Press Ctrl+N to write one.")
                    } else {
                        tr("Prova con meno parole o senza #tag.", "Try fewer words or no #tag.")
                    };
                    ui.label(RichText::new(hint).size(12.5).color(p.text_muted));
                });
            }
            ui.add_space(12.0);
        });

        if let Some((path, mods)) = clicked {
            self.click_note(&path, mods, &order);
        }
        if let Some(path) = pin {
            self.toggle_pin(&path);
        }
        if let Some((path, folder)) = moving {
            self.move_note(&path, &folder);
        }
        if let Some(path) = delete {
            let targets = self.delete_targets(Some(&path));
            self.request_delete(targets);
        }
    }

    /// Clic su una nota: normale = apri; Ctrl = aggiungi/togli dalla selezione;
    /// Maiusc = seleziona l'intervallo dall'ultima nota scelta.
    pub(super) fn click_note(&mut self, path: &Path, mods: Modifiers, order: &[PathBuf]) {
        if mods.command {
            if self.selection.is_empty()
                && let Some(open) = self.open_path().map(Path::to_path_buf)
                && !vault::same_path(&open, path)
            {
                self.selection.push(open);
            }
            if let Some(i) = self.selection.iter().position(|s| vault::same_path(s, path)) {
                self.selection.remove(i);
            } else {
                self.selection.push(path.to_path_buf());
            }
            self.anchor = Some(path.to_path_buf());
        } else if mods.shift {
            let from = self.anchor.clone().or_else(|| self.open_path().map(Path::to_path_buf));
            let a = from.and_then(|f| order.iter().position(|p| vault::same_path(p, &f))).unwrap_or(0);
            let b = order.iter().position(|p| vault::same_path(p, path)).unwrap_or(a);
            let (lo, hi) = (a.min(b), a.max(b));
            self.selection = order[lo..=hi].to_vec();
        } else {
            self.open_note(path);
        }
    }

    pub(super) fn editor_area(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        if self.open.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new(tr("Seleziona una nota o premi Ctrl+N", "Select a note or press Ctrl+N")).color(p.text_muted));
            });
            return;
        }
        let full = ui.available_rect_before_wrap();

        // Barra in alto: posizione della nota e azioni.
        let mut toolbar_action: Option<Command> = None;
        let (bar, _) = ui.allocate_exact_size(Vec2::new(full.width(), 44.0), Sense::hover());
        let open_path = self.open_path().map(Path::to_path_buf);
        let external = self.open.as_ref().is_some_and(|o| o.external);
        let root = match (&self.vault, &self.open) {
            (Some(v), Some(o)) if !o.external => v.root.clone(),
            (_, Some(o)) => o.dir.clone(),
            _ => PathBuf::new(),
        };
        ui.scope_builder(egui::UiBuilder::new().max_rect(bar.shrink2(Vec2::new(12.0, 6.0))), |ui| {
            ui.horizontal_centered(|ui| {
                if (!self.settings.sidebar_open || !self.settings.list_open)
                    && icons::button(ui, Icon::Sidebar, tr("Mostra le colonne (Ctrl+Shift+\\)", "Show columns (Ctrl+Shift+\\)")).clicked()
                {
                    self.settings.sidebar_open = true;
                    self.settings.list_open = true;
                }
                let crumb = match (&open_path, &self.open) {
                    (Some(path), _) if external => format!(
                        "{}  ·  {}",
                        tr("File esterno", "External file"),
                        widgets::middle_ellipsis(&path.parent().map(|d| d.display().to_string()).unwrap_or_default(), 60)
                    ),
                    (Some(path), _) => {
                        let dir = path.parent().map(|d| rel(&root, d)).unwrap_or_default();
                        if dir.is_empty() { tr("Note", "Notes").to_owned() } else { dir.replace('/', "  ›  ") }
                    }
                    (None, Some(o)) => {
                        let dir = rel(&root, &o.dir);
                        if dir.is_empty() { tr("Note", "Notes").to_owned() } else { dir.replace('/', "  ›  ") }
                    }
                    _ => String::new(),
                };
                ui.label(RichText::new(crumb).size(13.0).color(p.text_muted));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if open_path.is_some() {
                        if icons::button(ui, Icon::Trash, tr("Sposta nel Cestino", "Move to Recycle Bin")).clicked() {
                            toolbar_action = Some(Command::DeleteNote);
                        }
                        let pinned = open_path.as_deref().is_some_and(|op| self.is_pinned(op));
                        let tip = if external {
                            ""
                        } else if pinned {
                            tr("Sblocca (Ctrl+Shift+P)", "Unpin (Ctrl+Shift+P)")
                        } else {
                            tr("Fissa in alto (Ctrl+Shift+P)", "Pin to top (Ctrl+Shift+P)")
                        };
                        // I file esterni non si possono fissare: non fanno parte delle note.
                        if !external {
                            let r = icons::button(ui, Icon::Pin, tip);
                            if pinned {
                                ui.painter().circle_filled(r.rect.right_top() + Vec2::new(-6.0, 6.0), 3.0, p.accent);
                            }
                            if r.clicked() {
                                toolbar_action = Some(Command::PinNote);
                            }
                        }
                    }
                });
            });
        });
        if let Some(c) = toolbar_action {
            let ctx = ui.ctx().clone();
            self.run_command(&ctx, c);
            if self.open.is_none() {
                return;
            }
        }

        // Trova e sostituisci: barra in alto a destra, sopra la nota.
        if let Some(o) = &mut self.open
            && self.editor.find_bar(ui.ctx(), Pos2::new(full.max.x - 14.0, bar.max.y + 2.0), &mut o.text)
        {
            o.last_edit = Instant::now();
            o.edited = true;
        }

        let width = (full.width() - 96.0).clamp(240.0, self.settings.text_width);
        let backlinks: Vec<(PathBuf, String)> = self.backlinks();
        let pinned = open_path.as_deref().is_some_and(|op| self.is_pinned(op));
        let modified = open_path
            .as_deref()
            .and_then(|op| self.vault.as_ref()?.notes.iter().find(|n| vault::same_path(&n.path, op)))
            .map(|n| n.modified)
            .or_else(|| self.open.as_ref().filter(|o| o.path.is_some()).and_then(|o| o.mtime));

        let Some(o) = &mut self.open else { return };
        let mut action = None;
        let mut open_backlink = None;
        let show_backlinks = &mut self.show_backlinks;
        let editor = &mut self.editor;
        let settings = &self.settings;
        let (titles, tags): (&[String], &[String]) = match &self.vault {
            Some(v) => (&v.titles, &v.tag_names),
            None => (&[], &[]),
        };
        egui::ScrollArea::vertical().id_salt("editor-scroll").auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
                ui.vertical(|ui| {
                    ui.set_width(width);
                    // Data della nota, centrata in alto come in Notes.
                    let when = modified.map(long_date).unwrap_or_else(|| tr("Nuova nota", "New note").to_owned());
                    ui.vertical_centered(|ui| ui.label(RichText::new(when).size(12.5).color(p.text_muted)));
                    ui.add_space(14.0);
                    let cx = EditorCtx {
                        note_dir: &o.dir,
                        vault_root: &root,
                        titles,
                        tags,
                        font_size: settings.font_size,
                        line_spacing: settings.line_spacing,
                        width,
                        has_file: open_path.is_some(),
                        can_pin: !external,
                        pinned,
                    };
                    let r = editor.show(ui, &mut o.text, &cx);
                    if r.changed {
                        o.last_edit = Instant::now();
                        o.edited = true;
                    }
                    action = r.action;
                    // Spazio in fondo: si può cliccare sotto il testo per continuare a scrivere.
                    let rest = ui.allocate_response(Vec2::new(width, 120.0), Sense::click());
                    if rest.clicked() {
                        editor.request_focus();
                    }
                    if !backlinks.is_empty() {
                        ui.add_space(8.0);
                        let label = format!(
                            "{} {} {}",
                            tr("Citata in", "Linked from"),
                            backlinks.len(),
                            if backlinks.len() == 1 { tr("nota", "note") } else { tr("note", "notes") }
                        );
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::click());
                        let chevron = Rect::from_center_size(Pos2::new(r.min.x + 9.0, r.center().y), Vec2::splat(14.0));
                        let painter = ui.painter();
                        if resp.hovered() {
                            painter.rect_filled(r, 6.0, p.hover);
                        }
                        let angle = if *show_backlinks { std::f32::consts::FRAC_PI_2 } else { 0.0 };
                        egui::Image::new(Icon::ChevronRight.source())
                            .tint(p.text_muted)
                            .rotate(angle, Vec2::splat(0.5))
                            .paint_at(ui, chevron);
                        ui.painter().text(
                            Pos2::new(r.min.x + 24.0, r.center().y),
                            Align2::LEFT_CENTER,
                            label,
                            FontId::new(13.0, theme::family(true, false)),
                            p.text_muted,
                        );
                        if resp.clicked() {
                            *show_backlinks = !*show_backlinks;
                        }
                        if *show_backlinks {
                            for (path, title) in &backlinks {
                                let resp = widgets::nav_row(ui, Icon::Link, title, None, false, 1);
                                if resp.clicked() {
                                    open_backlink = Some(path.clone());
                                }
                            }
                        }
                    }
                    ui.add_space(60.0);
                });
            });
        });

        // Stato in basso a destra: parole e salvataggio.
        let words = o.text.split_whitespace().count();
        let status = if o.dirty() { tr("Modifiche in salvataggio…", "Saving…") } else { tr("Salvato", "Saved") };
        let words_label = if words == 1 { tr("parola", "word") } else { tr("parole", "words") };
        ui.painter().text(
            full.right_bottom() - Vec2::new(16.0, 12.0),
            Align2::RIGHT_BOTTOM,
            format!("{words} {words_label}  ·  {status}"),
            FontId::proportional(12.0),
            p.text_muted,
        );

        if let Some(path) = open_backlink {
            self.open_note(&path);
        }
        match action {
            Some(EditorAction::OpenUrl(u)) => {
                let url = if u.contains("://") || u.starts_with("mailto:") { u } else { format!("https://{u}") };
                ui.ctx().open_url(egui::OpenUrl::new_tab(url));
            }
            Some(EditorAction::OpenNote(t)) => self.open_or_create_by_title(&t),
            Some(EditorAction::FilterTag(t)) => {
                self.filter = Filter::Tag(t.to_lowercase());
                self.settings.list_open = true;
            }
            Some(EditorAction::Error(e)) => self.error = Some(e),
            Some(EditorAction::TogglePin) => {
                let ctx = ui.ctx().clone();
                self.run_command(&ctx, Command::PinNote);
            }
            Some(EditorAction::Delete) => {
                let ctx = ui.ctx().clone();
                self.run_command(&ctx, Command::DeleteNote);
            }
            Some(EditorAction::RevealInExplorer) => {
                if let Some(p) = self.open_path() {
                    crate::platform::reveal_file(p);
                }
            }
            Some(EditorAction::CopyPath) => {
                if let Some(p) = self.open_path() {
                    ui.ctx().copy_text(p.display().to_string());
                    self.toast = Some(Toast::new(
                        tr("Percorso copiato negli appunti", "Path copied to clipboard").into(),
                        Vec::new(),
                        Duration::from_secs(3),
                    ));
                }
            }
            None => {}
        }
    }

    /// Note che contengono un `[[link]]` alla nota aperta.
    pub(super) fn backlinks(&self) -> Vec<(PathBuf, String)> {
        let (Some(v), Some(path)) = (&self.vault, self.open_path()) else { return Vec::new() };
        let Some(me) = v.notes.iter().find(|n| vault::same_path(&n.path, path)) else { return Vec::new() };
        let names = [me.title.to_lowercase(), path.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()];
        v.notes
            .iter()
            .filter(|n| !vault::same_path(&n.path, path) && n.links.iter().any(|l| names.contains(l)))
            .map(|n| (n.path.clone(), n.title.clone()))
            .collect()
    }

    pub(super) fn error_banner(&mut self, ui: &mut egui::Ui) {
        let Some(msg) = self.error.clone() else { return };
        let p = pal(ui);
        egui::Panel::top("error")
            .frame(
                egui::Frame::new()
                    .fill(if p.dark { Color32::from_rgb(0x44, 0x27, 0x26) } else { Color32::from_rgb(0xFD, 0xE7, 0xE9) })
                    .inner_margin(egui::Margin::symmetric(14, 8)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(msg).color(if p.dark {
                        Color32::from_rgb(0xFF, 0xD7, 0xD5)
                    } else {
                        Color32::from_rgb(0x8A, 0x16, 0x0B)
                    }));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icons::button_sized(ui, Icon::Close, tr("Chiudi", "Dismiss"), 24.0, 14.0).clicked() {
                            self.error = None;
                        }
                    });
                });
            });
    }

    pub(super) fn toast_ui(&mut self, ctx: &egui::Context) {
        let Some(t) = &self.toast else { return };
        let now = Instant::now();
        if now >= t.until {
            self.toast = None;
            return;
        }
        // Entra salendo dal basso e svanisce alla fine (curva "ease-out").
        let shown = now.duration_since(t.created).as_secs_f32();
        let left = t.until.duration_since(now).as_secs_f32();
        let ease = |x: f32| 1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3);
        let k = ease(shown / TOAST_ANIM).min(ease(left / TOAST_ANIM));
        if k < 1.0 {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_secs_f32((left - TOAST_ANIM).max(0.05)));
        }
        let p = theme::palette(ctx);
        let mut undo = None;
        let mut dismiss = false;
        egui::Area::new(Id::unique("toast"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -28.0 + (1.0 - k) * 24.0))
            .show(ctx, |ui| {
                ui.set_opacity(k);
                egui::Frame::window(ui.style()).inner_margin(egui::Margin::symmetric(14, 8)).corner_radius(12).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if !t.undo.is_empty() {
                            icons::show(ui, Icon::Trash, 15.0, p.text_muted);
                        }
                        ui.label(RichText::new(&t.text).size(13.5).color(p.text));
                        if !t.undo.is_empty() {
                            ui.add_space(6.0);
                            if ui
                                .add(egui::Button::new(RichText::new(tr("Annulla", "Undo")).strong().color(p.accent_text)).frame(false))
                                .clicked()
                            {
                                undo = Some(t.undo.clone());
                            }
                        }
                        if icons::button_sized(ui, Icon::Close, tr("Chiudi", "Dismiss"), 22.0, 13.0).clicked() {
                            dismiss = true;
                        }
                    });
                });
            });
        if let Some(paths) = undo {
            self.undo_delete(&paths);
        } else if dismiss {
            self.toast = None;
        }
    }
}

const MONTHS_EN: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

const MONTHS_IT: [&str; 12] =
    ["gennaio", "febbraio", "marzo", "aprile", "maggio", "giugno", "luglio", "agosto", "settembre", "ottobre", "novembre", "dicembre"];

pub(super) fn flatten_folders(root: &vault::Folder) -> Vec<(PathBuf, String, usize)> {
    fn walk(f: &vault::Folder, depth: usize, out: &mut Vec<(PathBuf, String, usize)>) {
        for c in &f.children {
            out.push((c.path.clone(), c.name.clone(), depth));
            walk(c, depth + 1, out);
        }
    }
    let mut out = vec![(root.path.clone(), tr("Note (cartella principale)", "Notes (main folder)").to_owned(), 0)];
    walk(root, 1, &mut out);
    out
}

/// Il marchio dell'app: un foglio arrotondato con tre righe, nel colore d'accento.
/// Il marchio di Yonote: la Y su un quadrato arrotondato, in bianco e nero.
/// Nel tema scuro i colori si invertono, così il marchio resta sempre visibile.
pub(super) fn paint_app_mark(ui: &egui::Ui, r: Rect) {
    let p = pal(ui);
    let s = r.width();
    let (square, glyph) =
        if p.dark { (Color32::from_gray(242), Color32::from_gray(12)) } else { (Color32::from_gray(12), Color32::from_gray(250)) };
    ui.painter().rect_filled(r, s * 0.225, square);
    let g = Rect::from_center_size(r.center(), Vec2::splat(s * 0.56));
    egui::Image::new(icons::logo_glyph()).tint(glyph).paint_at(ui, g);
}

pub(super) fn list_group(ui: &mut egui::Ui, text: &str) {
    ui.add_space(10.0);
    let p = pal(ui);
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label(RichText::new(text).size(12.5).family(theme::family(true, false)).color(p.text_muted));
    });
    ui.add_space(2.0);
}

pub(super) fn local(t: SystemTime) -> chrono::DateTime<chrono::Local> {
    chrono::DateTime::<chrono::Local>::from(t)
}

pub(super) fn month_name(m0: u32) -> &'static str {
    if crate::i18n::is_english() { MONTHS_EN[m0 as usize] } else { MONTHS_IT[m0 as usize] }
}

pub(super) fn date_group(t: SystemTime) -> String {
    use chrono::Datelike;
    let today = chrono::Local::now().date_naive();
    let d = local(t).date_naive();
    match (today - d).num_days() {
        ..=0 => tr("Oggi", "Today").into(),
        1 => tr("Ieri", "Yesterday").into(),
        2..=7 => tr("Ultimi 7 giorni", "Previous 7 days").into(),
        8..=30 => tr("Ultimi 30 giorni", "Previous 30 days").into(),
        _ => {
            let m = month_name(d.month0());
            let mut c = m.chars();
            let cap: String = c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default();
            if d.year() == today.year() { cap } else { format!("{cap} {}", d.year()) }
        }
    }
}

pub(super) fn short_date(t: SystemTime) -> String {
    let dt = local(t);
    match (chrono::Local::now().date_naive() - dt.date_naive()).num_days() {
        ..=0 => dt.format("%H:%M").to_string(),
        1 => tr("Ieri", "Yesterday").into(),
        _ if crate::i18n::is_english() => dt.format("%m/%d/%y").to_string(),
        _ => dt.format("%d/%m/%y").to_string(),
    }
}

pub(super) fn long_date(t: SystemTime) -> String {
    use chrono::Datelike;
    let dt = local(t);
    if crate::i18n::is_english() {
        format!("{} {}, {} at {}", month_name(dt.month0()), dt.day(), dt.year(), dt.format("%H:%M"))
    } else {
        format!("{} {} {} alle {}", dt.day(), month_name(dt.month0()), dt.year(), dt.format("%H:%M"))
    }
}
