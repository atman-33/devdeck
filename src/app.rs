use crate::models::{now_unix, time_ago, Config, GitInfo, Preset, Project, Settings, SortMode};
use crate::{actions, git, storage};
use eframe::egui;
use egui::{Color32, RichText};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};

/// Messages sent back from background git worker threads.
enum Msg {
    Status(String, GitInfo),
    OpDone {
        path: String,
        op: &'static str,
        result: Result<String, String>,
    },
}

pub struct DevDeckApp {
    cfg: Config,
    dirty: bool,

    git: HashMap<String, GitInfo>,
    busy: HashMap<String, &'static str>,
    branch_sel: HashMap<String, String>,

    selected: HashSet<String>,

    search: String,
    tag_filter: String,
    favorites_only: bool,

    status_line: String,
    show_settings: bool,
    settings_draft: Settings,
    new_preset_name: String,

    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl DevDeckApp {
    pub fn new() -> Self {
        let cfg = storage::load();
        let selected: HashSet<String> = cfg.selected.iter().cloned().collect();
        let (tx, rx) = channel();
        let mut app = Self {
            settings_draft: cfg.settings.clone(),
            selected,
            cfg,
            dirty: false,
            git: HashMap::new(),
            busy: HashMap::new(),
            branch_sel: HashMap::new(),
            search: String::new(),
            tag_filter: String::new(),
            favorites_only: false,
            status_line: String::new(),
            show_settings: false,
            new_preset_name: String::new(),
            tx,
            rx,
        };
        app.refresh_all();
        app
    }

    // ---- background git ----

    fn refresh_status(&mut self, path: &str) {
        if self.busy.contains_key(path) {
            return;
        }
        self.busy.insert(path.to_string(), "status");
        let tx = self.tx.clone();
        let p = path.to_string();
        std::thread::spawn(move || {
            let info = git::read_status(&p);
            let _ = tx.send(Msg::Status(p, info));
        });
    }

    fn refresh_all(&mut self) {
        let paths: Vec<String> = self.cfg.projects.iter().map(|p| p.path.clone()).collect();
        for p in paths {
            self.refresh_status(&p);
        }
    }

    fn run_op(&mut self, path: &str, op: &'static str) {
        if self.busy.contains_key(path) {
            return;
        }
        self.busy.insert(path.to_string(), op);
        let tx = self.tx.clone();
        let p = path.to_string();
        let branch = self.branch_sel.get(path).cloned().unwrap_or_default();
        std::thread::spawn(move || {
            let result = match op {
                "fetch" => git::fetch(&p),
                "pull" => git::pull(&p),
                "switch" => git::switch(&p, &branch),
                _ => Err("unknown op".into()),
            };
            let _ = tx.send(Msg::OpDone { path: p, op, result });
        });
    }

    fn drain_messages(&mut self) {
        let mut refresh_after: Vec<String> = Vec::new();
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Status(path, info) => {
                    self.busy.remove(&path);
                    // Keep the branch selector in sync with reality.
                    if !info.branch.is_empty() {
                        self.branch_sel.insert(path.clone(), info.branch.clone());
                    }
                    self.git.insert(path, info);
                }
                Msg::OpDone { path, op, result } => {
                    self.busy.remove(&path);
                    let name = self.project_name(&path);
                    match result {
                        Ok(m) => self.status_line = format!("{name}: {op} ok — {m}"),
                        Err(e) => self.status_line = format!("{name}: {op} failed — {e}"),
                    }
                    refresh_after.push(path);
                }
            }
        }
        for p in refresh_after {
            self.refresh_status(&p);
        }
    }

    fn project_name(&self, path: &str) -> String {
        self.cfg
            .projects
            .iter()
            .find(|p| p.path == path)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| path.to_string())
    }

    // ---- mutations ----

    fn add_projects(&mut self) {
        if let Some(folders) = rfd::FileDialog::new()
            .set_title("Add project folders")
            .pick_folders()
        {
            for folder in folders {
                let path = folder.to_string_lossy().replace('\\', "/");
                if self.cfg.projects.iter().any(|p| p.path == path) {
                    continue;
                }
                let name = folder
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.clone());
                self.cfg.projects.push(Project {
                    path: path.clone(),
                    name,
                    tags: String::new(),
                    favorite: false,
                    notes: String::new(),
                    last_opened: None,
                });
                self.refresh_status(&path);
            }
            self.dirty = true;
        }
    }

    fn mark_opened(&mut self, paths: &[String]) {
        let now = now_unix();
        for p in &mut self.cfg.projects {
            if paths.contains(&p.path) {
                p.last_opened = Some(now);
            }
        }
        self.dirty = true;
    }

    fn selected_paths(&self) -> Vec<String> {
        // Preserve list order rather than HashSet order.
        self.cfg
            .projects
            .iter()
            .filter(|p| self.selected.contains(&p.path))
            .map(|p| p.path.clone())
            .collect()
    }

    fn open_selected_in_vscode(&mut self) {
        let paths = self.selected_paths();
        match actions::open_in_vscode(&self.cfg.settings.vscode_cmd, &paths) {
            Ok(()) => {
                self.status_line = format!("opened {} project(s) in VS Code", paths.len());
                self.mark_opened(&paths);
            }
            Err(e) => self.status_line = format!("VS Code launch failed — {e}"),
        }
    }

    // ---- UI ----

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("➕ Add projects").clicked() {
                self.add_projects();
            }
            if ui.button("🔄 Refresh").clicked() {
                self.refresh_all();
            }
            if ui.button("⬇ Fetch all").clicked() {
                let paths: Vec<String> =
                    self.cfg.projects.iter().map(|p| p.path.clone()).collect();
                for p in paths {
                    self.run_op(&p, "fetch");
                }
            }

            ui.separator();

            let n = self.selected.len();
            let open_btn = egui::Button::new(
                RichText::new(format!("🚀 Open in VS Code ({n})")).strong(),
            );
            if ui.add_enabled(n > 0, open_btn).clicked() {
                self.open_selected_in_vscode();
            }
            if n > 0 && ui.button("Clear selection").clicked() {
                self.selected.clear();
                self.dirty = true;
            }

            ui.separator();
            self.preset_controls(ui);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⚙ Settings").clicked() {
                    self.settings_draft = self.cfg.settings.clone();
                    self.show_settings = true;
                }
            });
        });

        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("filter by name/path")
                    .desired_width(180.0),
            );
            ui.label("🏷");
            ui.add(
                egui::TextEdit::singleline(&mut self.tag_filter)
                    .hint_text("filter by tag")
                    .desired_width(120.0),
            );
            ui.checkbox(&mut self.favorites_only, "★ favorites only");
            ui.separator();
            ui.label("Sort:");
            if ui
                .selectable_label(self.cfg.sort == SortMode::Name, "Name")
                .clicked()
            {
                self.cfg.sort = SortMode::Name;
                self.dirty = true;
            }
            if ui
                .selectable_label(self.cfg.sort == SortMode::Recent, "Recent")
                .clicked()
            {
                self.cfg.sort = SortMode::Recent;
                self.dirty = true;
            }
        });
    }

    fn preset_controls(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_id_salt("preset_load")
            .selected_text("📦 Presets")
            .width(110.0)
            .show_ui(ui, |ui| {
                let presets = self.cfg.presets.clone();
                if presets.is_empty() {
                    ui.label("(no presets yet)");
                }
                let mut delete: Option<usize> = None;
                for (i, preset) in presets.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui.button(&preset.name).clicked() {
                            self.selected = preset.paths.iter().cloned().collect();
                            self.status_line = format!("preset '{}' loaded", preset.name);
                            self.dirty = true;
                        }
                        if ui.small_button("🗑").clicked() {
                            delete = Some(i);
                        }
                    });
                }
                if let Some(i) = delete {
                    self.cfg.presets.remove(i);
                    self.dirty = true;
                }
            });
        ui.add(
            egui::TextEdit::singleline(&mut self.new_preset_name)
                .hint_text("preset name")
                .desired_width(110.0),
        );
        let can_save = !self.new_preset_name.trim().is_empty() && !self.selected.is_empty();
        if ui
            .add_enabled(can_save, egui::Button::new("💾 Save preset"))
            .clicked()
        {
            let name = self.new_preset_name.trim().to_string();
            let paths = self.selected_paths();
            self.cfg.presets.retain(|p| p.name != name);
            self.cfg.presets.push(Preset { name, paths });
            self.new_preset_name.clear();
            self.dirty = true;
        }
    }

    fn visible_projects(&self) -> Vec<usize> {
        let search = self.search.to_lowercase();
        let tag = self.tag_filter.trim().to_lowercase();
        let mut idx: Vec<usize> = self
            .cfg
            .projects
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                if self.favorites_only && !p.favorite {
                    return false;
                }
                if !search.is_empty()
                    && !p.name.to_lowercase().contains(&search)
                    && !p.path.to_lowercase().contains(&search)
                {
                    return false;
                }
                if !tag.is_empty()
                    && !p
                        .tags
                        .to_lowercase()
                        .split(',')
                        .any(|t| t.trim().contains(&tag))
                {
                    return false;
                }
                true
            })
            .map(|(i, _)| i)
            .collect();

        idx.sort_by(|&a, &b| {
            let pa = &self.cfg.projects[a];
            let pb = &self.cfg.projects[b];
            // Favorites always float to the top.
            pb.favorite.cmp(&pa.favorite).then_with(|| match self.cfg.sort {
                SortMode::Name => pa.name.to_lowercase().cmp(&pb.name.to_lowercase()),
                SortMode::Recent => pb.last_opened.unwrap_or(0).cmp(&pa.last_opened.unwrap_or(0)),
            })
        });
        idx
    }

    fn project_row(&mut self, ui: &mut egui::Ui, index: usize) -> RowAction {
        let mut action = RowAction::None;
        let path = self.cfg.projects[index].path.clone();
        let info = self.git.get(&path).cloned();
        let busy = self.busy.get(path.as_str()).copied();

        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::symmetric(8.0, 6.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // selection + favorite + name
                    let mut checked = self.selected.contains(&path);
                    if ui.checkbox(&mut checked, "").changed() {
                        if checked {
                            self.selected.insert(path.clone());
                        } else {
                            self.selected.remove(&path);
                        }
                        self.dirty = true;
                    }
                    let project = &mut self.cfg.projects[index];
                    let star = if project.favorite { "★" } else { "☆" };
                    if ui
                        .selectable_label(project.favorite, star)
                        .on_hover_text("favorite")
                        .clicked()
                    {
                        project.favorite = !project.favorite;
                        self.dirty = true;
                    }
                    ui.label(RichText::new(&project.name).strong().size(15.0));
                    if !project.tags.trim().is_empty() {
                        for t in project.tags.split(',').filter(|t| !t.trim().is_empty()) {
                            ui.label(
                                RichText::new(format!("#{}", t.trim()))
                                    .color(Color32::from_rgb(120, 160, 220))
                                    .size(11.0),
                            );
                        }
                    }
                    if let Some(ts) = project.last_opened {
                        ui.label(
                            RichText::new(time_ago(ts))
                                .color(Color32::GRAY)
                                .size(11.0),
                        );
                    }

                    // git badges (right-aligned)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(op) = busy {
                            ui.add(egui::Spinner::new().size(14.0));
                            ui.label(RichText::new(op).color(Color32::GRAY).size(11.0));
                        }
                        match &info {
                            None => {
                                ui.label(RichText::new("…").color(Color32::GRAY));
                            }
                            Some(i) if !i.is_repo => {
                                ui.label(
                                    RichText::new("not a git repo").color(Color32::GRAY),
                                );
                            }
                            Some(i) => {
                                if let Some(e) = &i.error {
                                    ui.label(RichText::new("⚠").color(Color32::RED))
                                        .on_hover_text(e);
                                }
                                if i.changes > 0 {
                                    ui.label(
                                        RichText::new(format!("● {} changes", i.changes))
                                            .color(Color32::from_rgb(230, 180, 60)),
                                    )
                                    .on_hover_text("uncommitted changes — work in progress");
                                } else {
                                    ui.label(
                                        RichText::new("clean")
                                            .color(Color32::from_rgb(110, 190, 120)),
                                    );
                                }
                                if i.behind > 0 {
                                    ui.label(
                                        RichText::new(format!("↓{} pull needed", i.behind))
                                            .color(Color32::from_rgb(240, 130, 70)),
                                    )
                                    .on_hover_text("remote has commits you don't — pull before starting work");
                                }
                                if i.ahead > 0 {
                                    ui.label(
                                        RichText::new(format!("↑{}", i.ahead))
                                            .color(Color32::from_rgb(120, 160, 220)),
                                    )
                                    .on_hover_text("local commits not pushed");
                                }
                                if i.has_upstream && i.ahead == 0 && i.behind == 0 {
                                    ui.label(
                                        RichText::new("✓ up to date")
                                            .color(Color32::from_rgb(110, 190, 120)),
                                    );
                                } else if !i.has_upstream && !i.detached {
                                    ui.label(
                                        RichText::new("no upstream").color(Color32::GRAY),
                                    );
                                }
                                ui.label(
                                    RichText::new(format!("⎇ {}", i.branch))
                                        .color(Color32::from_rgb(160, 140, 220))
                                        .strong(),
                                );
                            }
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&path).color(Color32::GRAY).size(11.0),
                    );
                });

                ui.horizontal_wrapped(|ui| {
                    if ui.button("VS Code").clicked() {
                        action = RowAction::OpenCode;
                    }
                    if ui.button("Terminal").clicked() {
                        action = RowAction::OpenTerminal;
                    }
                    if ui.button("🤖 Agent").on_hover_text("launch AI agent (claude)").clicked() {
                        action = RowAction::LaunchAgent;
                    }
                    if ui.button("📁 Explorer").clicked() {
                        action = RowAction::OpenExplorer;
                    }
                    let is_repo = info.as_ref().map(|i| i.is_repo).unwrap_or(false);
                    if is_repo {
                        ui.separator();
                        if ui.add_enabled(busy.is_none(), egui::Button::new("Fetch")).clicked() {
                            action = RowAction::Fetch;
                        }
                        if ui.add_enabled(busy.is_none(), egui::Button::new("Pull")).clicked() {
                            action = RowAction::Pull;
                        }
                        // branch switcher
                        if let Some(i) = &info {
                            if !i.branches.is_empty() && !i.detached {
                                let current =
                                    self.branch_sel.entry(path.clone()).or_insert_with(|| i.branch.clone());
                                let before = current.clone();
                                egui::ComboBox::from_id_salt(("branch", &path))
                                    .selected_text(format!("⎇ {before}"))
                                    .width(160.0)
                                    .show_ui(ui, |ui| {
                                        for b in &i.branches {
                                            ui.selectable_value(current, b.clone(), b);
                                        }
                                    });
                                let after = self.branch_sel.get(&path).cloned().unwrap_or_default();
                                if after != before && after != i.branch && busy.is_none() {
                                    action = RowAction::Switch;
                                }
                            }
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let project = &mut self.cfg.projects[index];
                        let has_notes = !project.notes.trim().is_empty();
                        let label = if has_notes { "📝 Notes*" } else { "📝 Notes" };
                        ui.menu_button(label, |ui| {
                            ui.set_min_width(320.0);
                            ui.label("Notes");
                            if ui
                                .add(
                                    egui::TextEdit::multiline(&mut project.notes)
                                        .desired_rows(5)
                                        .desired_width(300.0),
                                )
                                .changed()
                            {
                                self.dirty = true;
                            }
                            ui.label("Tags (comma separated)");
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut project.tags)
                                        .desired_width(300.0),
                                )
                                .changed()
                            {
                                self.dirty = true;
                            }
                        });
                        if ui
                            .small_button("✖")
                            .on_hover_text("remove from DevDeck (does not delete files)")
                            .clicked()
                        {
                            action = RowAction::Remove;
                        }
                    });
                });
            });
        action
    }

    fn apply_row_action(&mut self, path: String, action: RowAction) {
        match action {
            RowAction::None => {}
            RowAction::OpenCode => {
                match actions::open_in_vscode(&self.cfg.settings.vscode_cmd, &[path.clone()]) {
                    Ok(()) => {
                        self.status_line = format!("{}: opened in VS Code", self.project_name(&path));
                        self.mark_opened(&[path]);
                    }
                    Err(e) => self.status_line = format!("VS Code launch failed — {e}"),
                }
            }
            RowAction::OpenTerminal => {
                match actions::open_terminal(&self.cfg.settings.terminal_cmd, &path) {
                    Ok(()) => self.mark_opened(&[path]),
                    Err(e) => self.status_line = format!("terminal launch failed — {e}"),
                }
            }
            RowAction::LaunchAgent => {
                match actions::launch_agent(&self.cfg.settings.agent_cmd, &path) {
                    Ok(()) => {
                        self.status_line =
                            format!("{}: agent launched", self.project_name(&path));
                        self.mark_opened(&[path]);
                    }
                    Err(e) => self.status_line = format!("agent launch failed — {e}"),
                }
            }
            RowAction::OpenExplorer => {
                if let Err(e) = actions::open_explorer(&path.replace('/', "\\")) {
                    self.status_line = format!("explorer launch failed — {e}");
                }
            }
            RowAction::Fetch => self.run_op(&path, "fetch"),
            RowAction::Pull => self.run_op(&path, "pull"),
            RowAction::Switch => self.run_op(&path, "switch"),
            RowAction::Remove => {
                self.cfg.projects.retain(|p| p.path != path);
                self.selected.remove(&path);
                self.git.remove(&path);
                self.dirty = true;
            }
        }
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let mut open = true;
        let mut save = false;
        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("VS Code command:");
                ui.text_edit_singleline(&mut self.settings_draft.vscode_cmd);
                ui.add_space(6.0);
                ui.label("Terminal command ({path} = project path):");
                ui.text_edit_singleline(&mut self.settings_draft.terminal_cmd);
                ui.add_space(6.0);
                ui.label("AI agent command ({path} = project path):");
                ui.text_edit_singleline(&mut self.settings_draft.agent_cmd);
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        save = true;
                    }
                    if ui.button("Reset to defaults").clicked() {
                        self.settings_draft = Settings::default();
                    }
                });
            });
        if save {
            self.cfg.settings = self.settings_draft.clone();
            self.dirty = true;
            self.show_settings = false;
        } else if !open {
            self.show_settings = false;
        }
    }

    fn persist_if_dirty(&mut self) {
        if self.dirty {
            self.cfg.selected = self.selected_paths();
            storage::save(&self.cfg);
            self.dirty = false;
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum RowAction {
    None,
    OpenCode,
    OpenTerminal,
    LaunchAgent,
    OpenExplorer,
    Fetch,
    Pull,
    Switch,
    Remove,
}

impl eframe::App for DevDeckApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_messages();
        if !self.busy.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
        }

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(4.0);
            self.toolbar(ui);
            ui.add_space(4.0);
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status_line).size(11.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} projects · {} selected",
                            self.cfg.projects.len(),
                            self.selected.len()
                        ))
                        .color(Color32::GRAY)
                        .size(11.0),
                    );
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.cfg.projects.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(60.0);
                    ui.label(RichText::new("No projects yet").size(18.0).strong());
                    ui.add_space(8.0);
                    ui.label("Click “➕ Add projects” to register local repositories.");
                });
                return;
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let visible = self.visible_projects();
                    for i in visible {
                        let path = self.cfg.projects[i].path.clone();
                        let action = self.project_row(ui, i);
                        if action != RowAction::None {
                            self.apply_row_action(path, action);
                        }
                        ui.add_space(4.0);
                    }
                });
        });

        self.settings_window(ctx);
        self.persist_if_dirty();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.cfg.selected = self.selected_paths();
        storage::save(&self.cfg);
    }
}
