use std::path::PathBuf;
use std::time::Instant;

use egui::*;

use crate::config::AppConfig;
use crate::image_loader::ImageLoader;
use crate::markdown::cache::AstCache;
use crate::markdown::parser::MarkdownDoc;
use crate::selection::TextSelector;
use crate::theme::Theme;
use crate::viewport::ViewportState;
use std::sync::Arc;

struct LoadedFont {
    id: String,
    data: Arc<egui::FontData>,
}

#[derive(Default)]
struct FontResolver {
    db: Option<fontdb::Database>,
}

impl FontResolver {
    fn resolve(
        &mut self,
        configured_name: Option<&str>,
        configured_path: Option<&str>,
        fallback_names: &[&str],
    ) -> Option<LoadedFont> {
        if let Some(font) = Self::load_from_path(configured_path) {
            return Some(font);
        }

        if let Some(name) = configured_name {
            if let Some(font) = self.find_by_name(name) {
                return Some(font);
            }
            tracing::warn!("Configured font '{name}' was not found in system fonts");
        }

        for fallback in fallback_names {
            if let Some(font) = self.find_by_name(fallback) {
                return Some(font);
            }
        }

        None
    }

    fn load_from_path(configured_path: Option<&str>) -> Option<LoadedFont> {
        let path = std::path::Path::new(configured_path?);
        if !path.is_file() {
            tracing::warn!("Configured font path does not exist: {}", path.display());
            return None;
        }

        let data = match std::fs::read(path) {
            Ok(data) => data,
            Err(err) => {
                tracing::warn!("Failed to read font file {}: {}", path.display(), err);
                return None;
            }
        };

        Some(LoadedFont {
            id: format!("font-file:{}", path.to_string_lossy()),
            data: Arc::new(egui::FontData::from_owned(data)),
        })
    }

    fn find_by_name(&mut self, name: &str) -> Option<LoadedFont> {
        let db = self.system_db();
        let query = fontdb::Query {
            families: &[fontdb::Family::Name(name)],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };
        let id = db.query(&query)?;
        let face = db.face(id)?;
        let data = match &face.source {
            fontdb::Source::Binary(bin) => bin.as_ref().as_ref().to_vec(),
            fontdb::Source::File(path) => std::fs::read(path).ok()?,
            fontdb::Source::SharedFile(path, _) => std::fs::read(path).ok()?,
        };
        let family_name = face.families.first()?.0.clone();

        Some(LoadedFont {
            id: format!("font-family:{family_name}"),
            data: Arc::new(egui::FontData::from_owned(data)),
        })
    }

    fn system_db(&mut self) -> &mut fontdb::Database {
        self.db.get_or_insert_with(|| {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            db
        })
    }
}

fn proportional_fallbacks() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        &["Microsoft YaHei", "Segoe UI", "Arial"]
    }
    #[cfg(target_os = "macos")]
    {
        &["PingFang SC", "SF Pro Text", "Helvetica Neue"]
    }
    #[cfg(target_os = "linux")]
    {
        &[
            "Noto Sans CJK SC",
            "Noto Sans",
            "DejaVu Sans",
            "Liberation Sans",
        ]
    }
}

fn monospace_fallbacks() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        &["Cascadia Mono", "Consolas", "Courier New"]
    }
    #[cfg(target_os = "macos")]
    {
        &["SF Mono", "Menlo", "Monaco"]
    }
    #[cfg(target_os = "linux")]
    {
        &[
            "JetBrains Mono",
            "DejaVu Sans Mono",
            "Liberation Mono",
            "Monospace",
        ]
    }
}

fn install_font_override(
    fonts: &mut egui::FontDefinitions,
    family: egui::FontFamily,
    resolver: &mut FontResolver,
    configured_name: Option<&str>,
    configured_path: Option<&str>,
    fallback_names: &[&str],
) {
    if let Some(font) = resolver.resolve(configured_name, configured_path, fallback_names) {
        let family_fonts = fonts.families.entry(family).or_default();
        if !family_fonts.iter().any(|existing| existing == &font.id) {
            family_fonts.insert(0, font.id.clone());
        }
        fonts.font_data.insert(font.id, font.data);
    }
}

fn setup_fonts(ctx: &egui::Context, config: &AppConfig) {
    let mut fonts = egui::FontDefinitions::default();
    let mut resolver = FontResolver::default();

    install_font_override(
        &mut fonts,
        egui::FontFamily::Proportional,
        &mut resolver,
        config.ui_font_name.as_deref(),
        config.ui_font_path.as_deref(),
        proportional_fallbacks(),
    );
    install_font_override(
        &mut fonts,
        egui::FontFamily::Monospace,
        &mut resolver,
        config.code_font_name.as_deref(),
        config.code_font_path.as_deref(),
        monospace_fallbacks(),
    );

    ctx.set_fonts(fonts);
}

struct SimpleFileWatcher {
    path: Option<PathBuf>,
    last_modified: Option<std::time::SystemTime>,
    last_checked: Instant,
}

impl SimpleFileWatcher {
    fn new(path: Option<PathBuf>) -> Self {
        let last_modified = path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok());
        Self {
            path,
            last_modified,
            last_checked: Instant::now(),
        }
    }

    fn check(&mut self) -> bool {
        if self.last_checked.elapsed() < std::time::Duration::from_secs(2) {
            return false;
        }
        self.last_checked = Instant::now();

        if let Some(path) = &self.path {
            if let Ok(meta) = std::fs::metadata(path) {
                if let Ok(modified) = meta.modified() {
                    if Some(modified) != self.last_modified {
                        self.last_modified = Some(modified);
                        return true;
                    }
                }
            }
        }
        false
    }
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
    /// Simple file watcher
    file_watcher: SimpleFileWatcher,
    /// Flag to indicate if config needs saving
    config_needs_save: bool,
    /// Last time config was saved
    last_save_time: Instant,
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

        setup_fonts(&cc.egui_ctx, &config);

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

        let file_watcher = SimpleFileWatcher::new(file_path.clone());

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
            file_watcher,
            config_needs_save: false,
            last_save_time: Instant::now(),
        }
    }

    /// Load a new file (using AST cache)
    pub fn load_file(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                self.error_msg = None;
                self.doc = Some(self.ast_cache.get_or_parse(&path, &content));
                self.viewport.reset(0);
                if let Some(dir) = path.parent() {
                    self.image_loader.set_base_dir(dir.to_path_buf());
                }
                self.file_path = Some(path.clone());
                self.file_watcher = SimpleFileWatcher::new(Some(path.clone()));
                self.config.last_file = Some(path.to_string_lossy().to_string());
                self.save_config();
            }
            Err(e) => {
                self.error_msg = Some(format!("无法打开文件: {}", e));
            }
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
                    self.config
                        .window_x
                        .is_none_or(|x| (x - rect.min.x).abs() > 1.0)
                        || self
                            .config
                            .window_y
                            .is_none_or(|y| (y - rect.min.y).abs() > 1.0)
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

        // Debounce config saving (use Duration instead of f64)
        if self.config_needs_save
            && self.last_save_time.elapsed() > std::time::Duration::from_secs(1)
        {
            if let Err(e) = self.config.save() {
                tracing::error!("Config save failed: {}", e);
            }
            self.config_needs_save = false;
            self.last_save_time = Instant::now();
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

        // Handle file watcher polling (every 2 seconds)
        if self.file_watcher.check() {
            if let Some(path) = &self.file_path.clone() {
                self.load_file(path.clone());
                ctx.request_repaint();
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
