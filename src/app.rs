use std::path::PathBuf;

use egui::*;
use font_kit::family_name::FamilyName;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;

use std::sync::Arc;
use crate::config::AppConfig;
use crate::image_loader::ImageLoader;
use crate::markdown::cache::AstCache;
use crate::markdown::parser::MarkdownDoc;
use crate::selection::TextSelector;
use crate::theme::Theme;
use crate::viewport::ViewportState;
use notify_debouncer_full::{Debouncer, new_debouncer, DebouncedEvent, FileIdMap};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

fn load_system_font(name: Option<&str>) -> Option<(String, Vec<u8>)> {
    let source = SystemSource::new();

    let families = if let Some(name) = name {
        vec![FamilyName::Title(name.to_string())]
    } else {
        vec![
            FamilyName::Title("Microsoft YaHei".to_string()),
            FamilyName::Title("PingFang SC".to_string()),
            FamilyName::Title("Noto Sans CJK SC".to_string()),
            FamilyName::Title("Segoe UI".to_string()),
            FamilyName::SansSerif,
        ]
    };

    let handle = source
        .select_best_match(&families, &Properties::new())
        .ok()?;

    let data = handle.load().ok()?.copy_font_data()?.to_vec();
    let font_name = name.unwrap_or("system").to_string();

    Some((font_name, data))
}

fn setup_fonts(ctx: &egui::Context, font_name: Option<&str>) {
    let mut fonts = egui::FontDefinitions::empty();

    if let Some((name, data)) = load_system_font(font_name) {
        fonts.font_data.insert(
            name.clone(),
            std::sync::Arc::new(egui::FontData::from_owned(data)),
        );

        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, name.clone());

        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, name.clone());
    }

    ctx.set_fonts(fonts);
}

/// Main application state
pub struct MdViewApp {
    /// Current file path
    file_path: Option<PathBuf>,
    /// Parsed markdown document
    doc: Option<Arc<MarkdownDoc>>,
    /// Current theme
    theme: Theme,
    /// Base font size
    font_size: f32,
    /// Whether the window has been shown for the first time
    first_frame_shown: bool,
    /// Text selection state
    selector: TextSelector,
    /// Async image loader
    image_loader: ImageLoader,
    /// Viewport culling state
    viewport: ViewportState,
    /// AST cache for avoiding re-parsing
    ast_cache: AstCache,
    /// Error message to display
    error_msg: Option<String>,
    /// App configuration
    config: AppConfig,
    /// Whether window is maximized
    window_maximized: bool,
    /// File system event debouncer
    file_watcher: Debouncer<RecommendedWatcher, FileIdMap>,
    /// Receiver for file system events
    file_events_rx: std::sync::mpsc::Receiver<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>,
    /// Flag to indicate if config needs saving
    config_needs_save: bool,
    /// Last time config was saved
    last_save_time: f64,
    }
impl MdViewApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        doc: Option<MarkdownDoc>,
        file_path: Option<PathBuf>,
    ) -> Self {
        let config = AppConfig::load();

        let base_dir = file_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let mut image_loader = ImageLoader::new(base_dir.clone());
        image_loader.set_context(cc.egui_ctx.clone());

        setup_fonts(&cc.egui_ctx, config.font_name.as_deref());

        let font_size = config.font_size;

        // Find the theme from presets (returns static reference)
        let themes = Theme::from_config();
        let theme = if let Some(ref theme_name) = config.theme_name {
            themes
                .iter()
                .find(|t| t.name == *theme_name)
                .cloned()
                .unwrap_or_else(Theme::default_theme)
        } else {
            Theme::default_theme()
        };

        // Setup file watcher
        let (tx, rx) = std::sync::mpsc::channel();
        let mut debouncer = new_debouncer(
            std::time::Duration::from_millis(200), // debounce changes
            None,
            tx,
        ).unwrap();

        // Watch the base directory recursively if file_path is set, otherwise watch current dir
        if let Some(ref path) = file_path {
            if let Some(dir) = path.parent() {
                debouncer.watcher().watch(dir, RecursiveMode::Recursive).unwrap();
            } else {
                debouncer.watcher().watch(&base_dir, RecursiveMode::Recursive).unwrap();
            }
        } else {
            debouncer.watcher().watch(&base_dir, RecursiveMode::Recursive).unwrap();
        }


        let doc = doc.map(Arc::new);

        Self {
            file_path,
            doc,
            theme,
            font_size,
            first_frame_shown: false,
            selector: TextSelector::new(),
            image_loader,
            viewport: ViewportState::new(0),
            ast_cache: AstCache::default(),
            error_msg: None,
            config,
            window_maximized: false,
            file_watcher: debouncer,
            file_events_rx: rx,
            config_needs_save: false,
            last_save_time: 0.0,
        }
    }

    /// Load a new file (using AST cache)
    pub fn load_file(&mut self, path: PathBuf) {
        // Stop watching previous file's directory if any
        if let Some(old_path) = self.file_path.as_ref() {
            if let Some(old_dir) = old_path.parent() {
                let _ = self.file_watcher.watcher().unwatch(old_dir);
            }
        }

        if let Ok(content) = std::fs::read_to_string(&path) {
            self.error_msg = None;
            self.doc = Some(self.ast_cache.get_or_parse(&path, &content));
            if let Some(dir) = path.parent() {
                self.image_loader.set_base_dir(dir.to_path_buf());
                // Start watching new file's directory
                let _ = self.file_watcher.watcher().watch(dir, RecursiveMode::Recursive);
            }
            self.file_path = Some(path.clone());
            self.config.last_file = Some(path.to_string_lossy().to_string());
            self.save_config(); // Debounced save
        } else {
            self.error_msg = Some(format!("无法打开文件"));
        }
    }

    /// Save current settings to config
    pub fn save_config(&mut self) {
        self.config.theme_name = Some(self.theme.name.to_string());
        self.config.font_size = self.font_size;
        self.config_needs_save = true;
    }

    /// Apply theme to egui visuals
    fn apply_theme(&self, ctx: &Context) {
        let mut visuals = if self.theme.is_dark {
            Visuals::dark()
        } else {
            Visuals::light()
        };
        visuals.panel_fill = self.theme.background;
        visuals.extreme_bg_color = self.theme.background;
        visuals.widgets.inactive.bg_fill = self.theme.code_bg;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, self.theme.foreground);
        // Set table stripe color for Grid::striped
        if let Some(stripe) = self.theme.table_stripe_bg {
            visuals.faint_bg_color = stripe;
        }
        ctx.set_visuals(visuals);
    }
}

impl eframe::App for MdViewApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Track maximized state FIRST
        let currently_maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);

        // Get viewport info for position
        let viewport_rect = ctx.input(|i| i.viewport().outer_rect);

        // Save config when maximized state changes or window size/position changes
        if currently_maximized != self.window_maximized {
            self.window_maximized = currently_maximized;
            self.config.maximized = currently_maximized;

            // Save position when unmaximizing
            if !currently_maximized {
                if let Some(rect) = viewport_rect {
                    self.config.window_x = Some(rect.min.x);
                    self.config.window_y = Some(rect.min.y);
                }
            }
            self.save_config(); // Use debounced save
        } else if !currently_maximized {
            let size = ctx.available_rect();
            if size.width() > 0.0 && size.height() > 0.0 {
                let size_changed = size.width() != self.config.window_width
                    || size.height() != self.config.window_height;

                // Save if position not set yet OR position changed
                let need_save_pos =
                    self.config.window_x.is_none() || self.config.window_y.is_none();
                let pos_changed = if let Some(rect) = viewport_rect {
                    let has_changed = self
                        .config
                        .window_x
                        .map_or(true, |x| (x - rect.min.x).abs() > 1.0)
                        || self
                            .config
                            .window_y
                            .map_or(true, |y| (y - rect.min.y).abs() > 1.0);
                    has_changed
                } else {
                    false
                };

                if size_changed || need_save_pos || pos_changed {
                    self.config.window_width = size.width();
                    self.config.window_height = size.height();

                    if let Some(rect) = viewport_rect {
                        self.config.window_x = Some(rect.min.x);
                        self.config.window_y = Some(rect.min.y);
                    }

                    self.save_config();
                }
            }
        }

        self.apply_theme(ctx);
        if self.image_loader.poll() {
            ctx.request_repaint();
        }
        self.selector.clear_segments();

        // Debounce config saving
        let current_time = ctx.input(|i| i.time);
        if self.config_needs_save && current_time - self.last_save_time > 0.5 {
            let _ = self.config.save();
            self.config_needs_save = false;
            self.last_save_time = current_time;
        }

        if !self.first_frame_shown {
            self.first_frame_shown = true;
        }

        // Handle keyboard shortcuts
        let has_selection = self.selector.has_selection();
        ctx.input(|input| {
            if input.modifiers.ctrl {
                // Ctrl+C - Copy selected text
                if input.key_pressed(egui::Key::C) && has_selection {
                    self.selector.copy_to_clipboard();
                }
                // Ctrl+= - Increase font size
                if input.key_pressed(egui::Key::Equals) {
                    self.font_size = (self.font_size + 2.0).min(32.0);
                    self.save_config();
                }
                // Ctrl+- - Decrease font size
                if input.key_pressed(egui::Key::Minus) {
                    self.font_size = (self.font_size - 2.0).max(8.0);
                    self.save_config();
                }
                // Ctrl+0 - Reset font size
                if input.key_pressed(egui::Key::Num0) {
                    self.font_size = 16.0;
                    self.save_config();
                }
                // Ctrl+O - Open file directory
                if input.key_pressed(egui::Key::O) {
                    if let Some(path) = &self.file_path {
                        if let Some(dir) = path.parent() {
                            let _ = open::that(dir);
                        }
                    }
                }
                // Ctrl+T - Cycle theme
                if input.key_pressed(egui::Key::T) {
                    let themes = Theme::from_config();
                    let current_idx = themes.iter().position(|t| t.name == self.theme.name);
                    if let Some(idx) = current_idx {
                        let next_idx = (idx + 1) % themes.len();
                        self.theme = themes[next_idx].clone();
                        self.save_config();
                    }
                }
            }
        });

        // Handle file watcher events
        while let Ok(result) = self.file_events_rx.try_recv() {
            match result {
                Ok(events) => {
                    for event in events {
                        if event.kind.is_modify() {
                            // Check if any of the modified paths is our current file
                            if let Some(current_file) = &self.file_path {
                                if event.paths.contains(current_file) {
                                    self.load_file(current_file.clone());
                                    ctx.request_repaint();
                                }
                            }
                        }
                    }
                }
                Err(errors) => {
                    for error in errors {
                        tracing::error!("File watcher error: {}", error);
                    }
                }
            }
        }

        // Handle dropped files
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(dropped) = dropped_files.first() {
            if let Some(path) = &dropped.path {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if matches!(ext, "md" | "markdown" | "txt") {
                    self.load_file(path.clone());
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        ctx.send_viewport_cmd(ViewportCommand::Title(format!("{} — mdview", name)));
                    }
                }
            }
        }

        CentralPanel::default()
            .frame(Frame::NONE.fill(self.theme.background))
            .show(ctx, |ui| {
                if let Some(doc) = &self.doc {
                    crate::markdown::renderer::render_doc(
                        ui,
                        doc,
                        &self.theme,
                        self.font_size,
                        &mut self.image_loader,
                        &mut self.selector,
                        &mut self.viewport,
                    );
                } else if let Some(err) = self.error_msg.clone() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() / 3.0);
                        ui.label(
                            RichText::new("⚠ 无法打开文件")
                                .size(24.0)
                                .color(egui::Color32::from_rgb(220, 80, 60)),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(&err)
                                .size(14.0)
                                .color(self.theme.muted_text()),
                        );
                        ui.add_space(16.0);
                        if ui.button("拖入 .md 文件或从命令行打开").clicked() {
                            self.error_msg = None;
                        }
                    });
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() / 3.0);
                        ui.label(
                            RichText::new("mdview")
                                .size(32.0)
                                .color(self.theme.muted_text()),
                        );
                        ui.add_space(8.0);
                        if ui
                            .button("Drop a .md file here, or open from command line")
                            .clicked()
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Markdown", &["md", "markdown", "txt"])
                                .pick_file()
                            {
                                self.load_file(path);
                            } else if let Some(dir) =
                                self.file_path.as_ref().and_then(|p| p.parent())
                            {
                                let _ = open::that(dir);
                            }
                        }
                    });
                }
            });
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        let _ = self.config.save();
    }
}
