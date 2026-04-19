use std::path::PathBuf;

use egui::*;

use crate::config::AppConfig;
use crate::image_loader::ImageLoader;
use crate::markdown::cache::AstCache;
use crate::markdown::parser::MarkdownDoc;
use crate::selection::TextSelector;
use crate::theme::Theme;
use crate::viewport::ViewportState;

/// Main application state
pub struct MdViewApp {
    /// Current file path
    file_path: Option<PathBuf>,
    /// Parsed markdown document
    doc: Option<MarkdownDoc>,
    /// Current theme
    theme: &'static Theme,
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
    /// Last file modification time
    last_mtime: Option<std::time::SystemTime>,
    /// Last file check time
    last_check_time: f64,
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

        let mut image_loader = ImageLoader::new(base_dir);
        image_loader.set_context(cc.egui_ctx.clone());

        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "microsoft_yahei".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../fonts/msyh.ttc"
            ))),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "microsoft_yahei".to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "microsoft_yahei".to_owned());
        cc.egui_ctx.set_fonts(fonts);

        let font_size = config.font_size;

        // Find the theme from presets (returns static reference)
        let theme = if let Some(ref theme_name) = config.theme_name {
            Theme::all_themes()
                .iter()
                .find(|t| t.name == theme_name)
                .unwrap_or_else(Theme::default_theme)
        } else {
            Theme::default_theme()
        };

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
            last_mtime: None,
            last_check_time: 0.0,
            window_maximized: false,
        }
    }

    /// Load a new file (using AST cache)
    pub fn load_file(&mut self, path: PathBuf) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            self.error_msg = None;
            self.doc = Some(self.ast_cache.get_or_parse(&path, &content));
            if let Some(dir) = path.parent() {
                self.image_loader.set_base_dir(dir.to_path_buf());
            }
            self.file_path = Some(path.clone());
            self.config.last_file = Some(path.to_string_lossy().to_string());
            let _ = self.config.save();
            // Get file mtime for change detection
            if let Ok(metadata) = std::fs::metadata(&path) {
                self.last_mtime = metadata.modified().ok();
            }
            self.last_check_time = 0.0;
        } else {
            self.error_msg = Some(format!("无法打开文件"));
        }
    }

    /// Save current settings to config
    pub fn save_config(&mut self) {
        self.config.theme_name = Some(self.theme.name.to_string());
        self.config.font_size = self.font_size;
        let _ = self.config.save();
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
            let _ = self.config.save();
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

                    let _ = self.config.save();
                }
            }
        }

        self.apply_theme(ctx);
        if self.image_loader.poll() {
            ctx.request_repaint();
        }
        self.selector.clear_segments();

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
                    let themes = crate::theme::Theme::all_themes();
                    let current_idx = themes.iter().position(|t| std::ptr::eq(t, self.theme));
                    if let Some(idx) = current_idx {
                        let next_idx = (idx + 1) % themes.len();
                        self.theme = &themes[next_idx];
                        self.save_config();
                    }
                }
            }
        });

        // File modification check (every 500ms)
        let time = ctx.input(|i| i.time);
        if time - self.last_check_time > 0.5 {
            self.last_check_time = time;
            if let Some(ref path) = self.file_path {
                if let Ok(metadata) = std::fs::metadata(path) {
                    if let Ok(mtime) = metadata.modified() {
                        if self.last_mtime.map_or(true, |lm| lm != mtime) {
                            self.load_file(path.clone());
                        }
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
                    let scroll_output = ScrollArea::vertical()
                        .id_salt(("mdview_scroll", self.file_path.clone()))
                        .auto_shrink([false, false])
                        .drag_to_scroll(false)
                        .show(ui, |ui| {
                            ui.horizontal_top(|ui| {
                                let total_width = ui.available_width();
                                let max_width = 800.0;
                                let content_width = total_width.min(max_width);
                                let margin = (total_width - content_width) / 2.0;
                                ui.add_space(margin);
                                ui.vertical(|ui| {
                                    ui.set_max_width(content_width);
                                    ui.add_space(16.0);

                                    crate::markdown::renderer::render_doc(
                                        ui,
                                        doc,
                                        self.theme,
                                        self.font_size,
                                        &mut self.image_loader,
                                        &mut self.selector,
                                        &mut self.viewport,
                                    );
                                });
                                ui.add_space(margin);
                            });
                        });
                    self.viewport.scroll_offset = scroll_output.state.offset.y;
                    self.viewport.viewport_height = scroll_output.inner_rect.height();

                    // Use click for right-click detection (doesn't block scrollbar much)
                    let area_response = ui.interact(
                        ui.max_rect(),
                        egui::Id::new("mdview_content"),
                        Sense::click(),
                    );

                    // Handle text selection via raw input events (for copy)
                    self.selector
                        .handle_input_raw(ctx, self.viewport.scroll_offset);
                    // Don't draw - use egui's built-in selection highlight

                    // Right-click context menu - check for right-click without dragging
                    area_response.context_menu(|ui| {
                        if ui
                            .add_enabled(
                                self.selector.has_selection(),
                                egui::Button::new("复制文本"),
                            )
                            .clicked()
                        {
                            self.selector.copy_to_clipboard();
                            ui.close_menu();
                        }
                        ui.separator();
                        ui.menu_button("字体大小", |ui| {
                            for size in [12.0, 14.0, 16.0, 18.0, 20.0] {
                                let label = format!("{}px", size as i32);
                                if self.font_size == size {
                                    ui.label(RichText::new(format!("▪ {}", label)).strong());
                                } else if ui.button(&label).clicked() {
                                    self.font_size = size;
                                    self.save_config();
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.menu_button("切换主题", |ui| {
                            for theme in crate::theme::Theme::all_themes() {
                                if std::ptr::eq(theme, self.theme) {
                                    ui.label(RichText::new(format!("▪ {}", theme.name)).strong());
                                } else if ui.button(theme.name).clicked() {
                                    self.theme = theme;
                                    self.save_config();
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.separator();
                        if ui.button("打开文件目录").clicked() {
                            if let Some(path) = &self.file_path {
                                if let Some(dir) = path.parent() {
                                    let _ = open::that(dir);
                                }
                            }
                            ui.close_menu();
                        }
                    });
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
