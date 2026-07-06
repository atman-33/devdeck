use crate::models::{now_unix, time_ago, Config, GitInfo, Preset, Project, Settings, SortMode};
use crate::{actions, git, storage, theme, update};
use eframe::egui;
use egui::RichText;
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
    UpdateAvailable {
        tag: String,
        url: String,
    },
    UpdateApplied(Result<(), String>),
}

/// UI state of the self-update flow.
enum UpdateUi {
    Hidden,
    Available { tag: String, url: String },
    Downloading { tag: String },
    Failed(String),
    RestartPending,
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

    update_ui: UpdateUi,

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
            update_ui: UpdateUi::Hidden,
            tx,
            rx,
        };
        app.refresh_all();
        app.spawn_update_check();
        app
    }

    fn spawn_update_check(&self) {
        update::cleanup_old();
        if !self.cfg.settings.check_updates {
            return;
        }
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            if let Some((tag, url)) = update::check_latest() {
                if update::is_newer(&tag, update::current_version()) {
                    let _ = tx.send(Msg::UpdateAvailable { tag, url });
                }
            }
        });
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
            let _ = tx.send(Msg::OpDone {
                path: p,
                op,
                result,
            });
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
                Msg::UpdateAvailable { tag, url } => {
                    self.update_ui = UpdateUi::Available { tag, url };
                }
                Msg::UpdateApplied(result) => {
                    self.update_ui = match result {
                        Ok(()) => UpdateUi::RestartPending,
                        Err(e) => UpdateUi::Failed(e),
                    };
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

    fn toggle_selected(&mut self, path: &str) {
        if !self.selected.remove(path) {
            self.selected.insert(path.to_string());
        }
        self.dirty = true;
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

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("🗀").size(22.0).color(theme::ACCENT));
            ui.label(
                RichText::new("DevDeck")
                    .size(19.0)
                    .strong()
                    .color(theme::TEXT),
            );
            ui.label(
                RichText::new(format!(
                    "developer workspace manager · v{}",
                    update::current_version()
                ))
                .size(11.5)
                .color(theme::TEXT_DIM),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(RichText::new("⚙").size(16.0))
                    .on_hover_text("Settings")
                    .clicked()
                {
                    self.settings_draft = self.cfg.settings.clone();
                    self.show_settings = true;
                }
                let n = self.selected.len();
                let label = if n <= 1 {
                    "▶ Open in VS Code".to_string()
                } else {
                    format!("▶ Open {n} in VS Code")
                };
                if theme::primary_button(ui, n > 0, &label)
                    .on_hover_text("open the selected projects as one VS Code workspace")
                    .clicked()
                {
                    self.open_selected_in_vscode();
                }
                if n > 0 {
                    if ui
                        .button("✖ Clear")
                        .on_hover_text("clear selection")
                        .clicked()
                    {
                        self.selected.clear();
                        self.dirty = true;
                    }
                    ui.label(
                        RichText::new(format!("{n} selected"))
                            .color(theme::ACCENT)
                            .strong(),
                    );
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("➕ Add")
                .on_hover_text("register local project folders")
                .clicked()
            {
                self.add_projects();
            }
            if ui
                .button("🔄 Refresh")
                .on_hover_text("re-read git status of all projects")
                .clicked()
            {
                self.refresh_all();
            }
            if ui
                .button("⬇ Fetch all")
                .on_hover_text("git fetch every project (updates the pull-needed badges)")
                .clicked()
            {
                let paths: Vec<String> = self.cfg.projects.iter().map(|p| p.path.clone()).collect();
                for p in paths {
                    self.run_op(&p, "fetch");
                }
            }

            ui.separator();
            self.preset_controls(ui);

            ui.separator();

            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("🔍 search name / path")
                    .desired_width(170.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.tag_filter)
                    .hint_text("🏷 tag")
                    .desired_width(90.0),
            );
            if ui
                .selectable_label(self.favorites_only, "★ favorites")
                .clicked()
            {
                self.favorites_only = !self.favorites_only;
            }

            ui.separator();
            ui.label(RichText::new("sort").color(theme::TEXT_DIM).size(11.5));
            if ui
                .selectable_label(self.cfg.sort == SortMode::Name, "name")
                .clicked()
            {
                self.cfg.sort = SortMode::Name;
                self.dirty = true;
            }
            if ui
                .selectable_label(self.cfg.sort == SortMode::Recent, "recent")
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
                    ui.label(RichText::new("no presets yet").color(theme::TEXT_DIM));
                    ui.label(
                        RichText::new("select projects, then save one →")
                            .color(theme::TEXT_DIM)
                            .size(11.0),
                    );
                }
                let mut delete: Option<usize> = None;
                for (i, preset) in presets.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui
                            .button(format!("{} ({})", preset.name, preset.paths.len()))
                            .clicked()
                        {
                            self.selected = preset.paths.iter().cloned().collect();
                            self.status_line = format!("preset '{}' loaded", preset.name);
                            self.dirty = true;
                        }
                        if ui
                            .small_button("🗑")
                            .on_hover_text("delete preset")
                            .clicked()
                        {
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
                .desired_width(100.0),
        );
        let can_save = !self.new_preset_name.trim().is_empty() && !self.selected.is_empty();
        if ui
            .add_enabled(can_save, egui::Button::new("💾 Save"))
            .on_hover_text("save the current selection as a preset")
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
            pb.favorite
                .cmp(&pa.favorite)
                .then_with(|| match self.cfg.sort {
                    SortMode::Name => pa.name.to_lowercase().cmp(&pb.name.to_lowercase()),
                    SortMode::Recent => pb
                        .last_opened
                        .unwrap_or(0)
                        .cmp(&pa.last_opened.unwrap_or(0)),
                })
        });
        idx
    }

    /// Status chips summarizing "is it safe to start working here?".
    fn status_chips(&self, ui: &mut egui::Ui, info: Option<&GitInfo>, busy: Option<&'static str>) {
        if let Some(op) = busy {
            ui.add(egui::Spinner::new().size(13.0).color(theme::ACCENT));
            ui.label(RichText::new(op).color(theme::TEXT_DIM).size(11.0));
        }
        let Some(i) = info else {
            theme::chip(ui, "loading…", theme::TEXT_DIM, theme::GRAY_BG);
            return;
        };
        if !i.is_repo {
            theme::chip(ui, "no git", theme::TEXT_DIM, theme::GRAY_BG);
            return;
        }
        if let Some(e) = &i.error {
            theme::chip(ui, "⚠ error", theme::RED, theme::RED_BG).on_hover_text(e);
        }

        // chips are laid out right-to-left: put the branch last so it renders leftmost
        let safe = i.changes == 0 && i.behind == 0 && i.error.is_none();
        if safe {
            theme::chip(ui, "✔ ready", theme::GREEN, theme::GREEN_BG)
                .on_hover_text("clean and up to date — safe to start working");
        }
        if i.ahead > 0 {
            theme::chip(
                ui,
                &format!("↑{} unpushed", i.ahead),
                theme::BLUE,
                theme::BLUE_BG,
            )
            .on_hover_text("local commits not pushed yet");
        }
        if i.behind > 0 {
            theme::chip(
                ui,
                &format!("↓{} pull needed", i.behind),
                theme::ORANGE,
                theme::ORANGE_BG,
            )
            .on_hover_text("remote has commits you don't — pull before starting work");
        }
        if i.changes > 0 {
            theme::chip(
                ui,
                &format!("● {} uncommitted", i.changes),
                theme::AMBER,
                theme::AMBER_BG,
            )
            .on_hover_text("uncommitted changes — this repo is mid-work");
        }
        if !i.has_upstream && !i.detached {
            theme::chip(ui, "no upstream", theme::TEXT_DIM, theme::GRAY_BG)
                .on_hover_text("branch has no remote tracking branch");
        }
        theme::chip(ui, &i.branch.to_string(), theme::PURPLE, theme::PURPLE_BG)
            .on_hover_text("current branch");
    }

    fn project_row(&mut self, ui: &mut egui::Ui, index: usize) -> RowAction {
        let mut action = RowAction::None;
        let path = self.cfg.projects[index].path.clone();
        let info = self.git.get(&path).cloned();
        let busy = self.busy.get(path.as_str()).copied();
        let is_selected = self.selected.contains(&path);

        let (fill, stroke) = if is_selected {
            (theme::CARD_SELECTED, egui::Stroke::new(1.5, theme::ACCENT))
        } else {
            (theme::CARD, egui::Stroke::new(1.0, theme::CARD_BORDER))
        };

        egui::Frame::none()
            .fill(fill)
            .stroke(stroke)
            .rounding(egui::Rounding::same(10.0))
            .inner_margin(egui::Margin {
                left: 12.0,
                right: 12.0,
                top: 9.0,
                bottom: 9.0,
            })
            .show(ui, |ui| {
                // -- row 1: select / name / chips --
                ui.horizontal(|ui| {
                    let mut checked = is_selected;
                    if ui
                        .checkbox(&mut checked, "")
                        .on_hover_text("select for VS Code")
                        .changed()
                    {
                        self.toggle_selected(&path);
                    }
                    let project = &mut self.cfg.projects[index];
                    let star = if project.favorite { "★" } else { "☆" };
                    let star_color = if project.favorite {
                        theme::AMBER
                    } else {
                        theme::TEXT_DIM
                    };
                    if ui
                        .button(RichText::new(star).color(star_color).size(15.0))
                        .on_hover_text("favorite (pinned to top)")
                        .clicked()
                    {
                        project.favorite = !project.favorite;
                        self.dirty = true;
                    }
                    // clicking the name also toggles selection — bigger target
                    if ui
                        .add(
                            egui::Label::new(
                                RichText::new(&project.name)
                                    .strong()
                                    .size(15.5)
                                    .color(theme::TEXT),
                            )
                            .sense(egui::Sense::click()),
                        )
                        .on_hover_text("click to select / deselect")
                        .clicked()
                    {
                        self.toggle_selected(&path);
                    }
                    let project = &self.cfg.projects[index];
                    for t in project.tags.split(',').filter(|t| !t.trim().is_empty()) {
                        theme::chip(ui, &format!("#{}", t.trim()), theme::ACCENT, theme::GRAY_BG);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        self.status_chips(ui, info.as_ref(), busy);
                    });
                });

                // -- row 2: path + last opened --
                ui.horizontal(|ui| {
                    ui.add_space(2.0);
                    ui.label(RichText::new(&path).color(theme::TEXT_DIM).size(11.0));
                    if let Some(ts) = self.cfg.projects[index].last_opened {
                        ui.label(
                            RichText::new(format!("· opened {}", time_ago(ts)))
                                .color(theme::TEXT_DIM)
                                .size(11.0),
                        );
                    }
                });

                ui.add_space(2.0);

                // -- row 3: actions --
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(RichText::new("💻 Code").color(theme::ACCENT))
                        .on_hover_text("open this project in VS Code")
                        .clicked()
                    {
                        action = RowAction::OpenCode;
                    }
                    if ui
                        .button("＞ Terminal")
                        .on_hover_text("open a terminal here")
                        .clicked()
                    {
                        action = RowAction::OpenTerminal;
                    }
                    if ui
                        .button("⚡ Agent")
                        .on_hover_text("launch the AI agent (claude) here")
                        .clicked()
                    {
                        action = RowAction::LaunchAgent;
                    }
                    if ui
                        .button("🗀 Files")
                        .on_hover_text("open in Explorer")
                        .clicked()
                    {
                        action = RowAction::OpenExplorer;
                    }
                    let is_repo = info.as_ref().map(|i| i.is_repo).unwrap_or(false);
                    if is_repo {
                        ui.separator();
                        if ui
                            .add_enabled(busy.is_none(), egui::Button::new("⬇ Fetch"))
                            .on_hover_text("git fetch — update remote state")
                            .clicked()
                        {
                            action = RowAction::Fetch;
                        }
                        if ui
                            .add_enabled(busy.is_none(), egui::Button::new("📥 Pull"))
                            .on_hover_text("git pull --ff-only")
                            .clicked()
                        {
                            action = RowAction::Pull;
                        }
                        // branch switcher
                        if let Some(i) = &info {
                            if !i.branches.is_empty() && !i.detached {
                                let current = self
                                    .branch_sel
                                    .entry(path.clone())
                                    .or_insert_with(|| i.branch.clone());
                                let before = current.clone();
                                egui::ComboBox::from_id_salt(("branch", &path))
                                    .selected_text(before.clone())
                                    .width(150.0)
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
                        if ui
                            .button(RichText::new("✖").color(theme::TEXT_DIM))
                            .on_hover_text("remove from DevDeck (does not delete files)")
                            .clicked()
                        {
                            action = RowAction::Remove;
                        }
                        let project = &mut self.cfg.projects[index];
                        let has_notes = !project.notes.trim().is_empty();
                        let label = if has_notes {
                            RichText::new("📝 Notes ●").color(theme::AMBER)
                        } else {
                            RichText::new("📝 Notes").color(theme::TEXT_DIM)
                        };
                        ui.menu_button(label, |ui| {
                            ui.set_min_width(320.0);
                            ui.label(RichText::new("Notes").color(theme::TEXT_DIM).size(11.5));
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
                            ui.label(
                                RichText::new("Tags (comma separated)")
                                    .color(theme::TEXT_DIM)
                                    .size(11.5),
                            );
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
                    });
                });
            });
        action
    }

    fn apply_row_action(&mut self, path: String, action: RowAction) {
        match action {
            RowAction::None => {}
            RowAction::OpenCode => {
                match actions::open_in_vscode(
                    &self.cfg.settings.vscode_cmd,
                    std::slice::from_ref(&path),
                ) {
                    Ok(()) => {
                        self.status_line =
                            format!("{}: opened in VS Code", self.project_name(&path));
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
                        self.status_line = format!("{}: agent launched", self.project_name(&path));
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
                ui.add_space(6.0);
                ui.checkbox(
                    &mut self.settings_draft.check_updates,
                    "Check for updates on startup",
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if theme::primary_button(ui, true, "Save").clicked() {
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

    fn empty_state(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(90.0);
            ui.label(RichText::new("🗀").size(48.0).color(theme::TEXT_DIM));
            ui.add_space(6.0);
            ui.label(
                RichText::new("No projects yet")
                    .size(19.0)
                    .strong()
                    .color(theme::TEXT),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Register local repositories to see their git status at a glance.")
                    .color(theme::TEXT_DIM),
            );
            ui.add_space(14.0);
            if theme::primary_button(ui, true, "➕ Add projects").clicked() {
                self.add_projects();
            }
        });
    }

    fn update_banner(&mut self, ctx: &egui::Context) {
        let mut next: Option<UpdateUi> = None;
        let mut start_download: Option<String> = None;
        let mut restart = false;

        let show = !matches!(self.update_ui, UpdateUi::Hidden);
        if !show {
            return;
        }
        egui::TopBottomPanel::top("update_banner")
            .frame(
                egui::Frame::none()
                    .fill(theme::ACCENT_DARK)
                    .inner_margin(egui::Margin::symmetric(14.0, 7.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| match &self.update_ui {
                    UpdateUi::Hidden => {}
                    UpdateUi::Available { tag, url } => {
                        ui.label(
                            RichText::new(format!(
                                "⬆ New version {tag} is available (current v{})",
                                update::current_version()
                            ))
                            .color(egui::Color32::WHITE)
                            .strong(),
                        );
                        if ui.button("Update & restart").clicked() {
                            start_download = Some(url.clone());
                            next = Some(UpdateUi::Downloading { tag: tag.clone() });
                        }
                        if ui.button("Later").clicked() {
                            next = Some(UpdateUi::Hidden);
                        }
                    }
                    UpdateUi::Downloading { tag } => {
                        ui.add(egui::Spinner::new().size(14.0).color(egui::Color32::WHITE));
                        ui.label(
                            RichText::new(format!("downloading {tag}…"))
                                .color(egui::Color32::WHITE),
                        );
                    }
                    UpdateUi::RestartPending => {
                        ui.label(
                            RichText::new("✔ Update installed")
                                .color(egui::Color32::WHITE)
                                .strong(),
                        );
                        if ui.button("Restart now").clicked() {
                            restart = true;
                        }
                    }
                    UpdateUi::Failed(e) => {
                        ui.label(
                            RichText::new(format!("update failed: {e}"))
                                .color(egui::Color32::WHITE),
                        );
                        if ui.button("✖").clicked() {
                            next = Some(UpdateUi::Hidden);
                        }
                    }
                });
            });

        if let Some(url) = start_download {
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(Msg::UpdateApplied(update::apply_update(&url)));
            });
        }
        if let Some(n) = next {
            self.update_ui = n;
        }
        if restart {
            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new(exe).spawn();
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
        if !self.busy.is_empty() || matches!(self.update_ui, UpdateUi::Downloading { .. }) {
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
        }

        self.update_banner(ctx);

        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::none()
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(14.0, 10.0)),
            )
            .show(ctx, |ui| {
                self.header(ui);
                ui.add_space(6.0);
                self.toolbar(ui);
            });

        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::none()
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(14.0, 5.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&self.status_line)
                            .size(11.0)
                            .color(theme::TEXT_DIM),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} projects · {} selected",
                                self.cfg.projects.len(),
                                self.selected.len()
                            ))
                            .color(theme::TEXT_DIM)
                            .size(11.0),
                        );
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(14.0, 12.0)),
            )
            .show(ctx, |ui| {
                if self.cfg.projects.is_empty() {
                    self.empty_state(ui);
                    return;
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let visible = self.visible_projects();
                        if visible.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(60.0);
                                ui.label(
                                    RichText::new("No projects match the current filter.")
                                        .color(theme::TEXT_DIM),
                                );
                            });
                        }
                        for i in visible {
                            let path = self.cfg.projects[i].path.clone();
                            let action = self.project_row(ui, i);
                            if action != RowAction::None {
                                self.apply_row_action(path, action);
                            }
                            ui.add_space(6.0);
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
