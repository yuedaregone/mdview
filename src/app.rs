use std::path::PathBuf;

use egui::*;

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
}

impl MdViewApp {
    pub fn new(cc: &eframe::CreationContext<'_>, doc: Option<MarkdownDoc>, file_path: Option<PathBuf>) -> Self {
        let base_dir = file_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let mut image_loader = ImageLoader::new(base_dir);
        image_loader.set_context(cc.egui_ctx.clone());

        Self {
            file_path,
            doc,
            theme: Theme::default_theme(),
            font_size: 16.0,
            first_frame_shown: false,
            selector: TextSelector::new(),
            image_loader,
            viewport: ViewportState::new(0),
            ast_cache: AstCache::default(),
            error_msg: None,
        }
    }

    /// Load a new file (using AST cache)
    pub fn load_file(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                self.error_msg = None;
                self.doc = Some(self.ast_cache.get_or_parse(&path, &content));
                if let Some(dir) = path.parent() {
                    self.image_loader.set_base_dir(dir.to_path_buf());
                }
                self.file_path = Some(path);
            }
            Err(e) => {
                self.error_msg = Some(format!("无法打开文件: {}", e));
            }
        }
    }

    /// Copy selected text to clipboard
    fn copy_selected_text(&self) {
        self.selector.copy_to_clipboard();
    }

    /// Open the directory containing the current file
    fn open_file_directory(&self) {
        if let Some(path) = &self.file_path {
            if let Some(dir) = path.parent() {
                let _ = open::that(dir);
            }
        }
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
        self.apply_theme(ctx);
        if self.image_loader.poll() {
            ctx.request_repaint();
        }
        self.selector.clear_segments();

        if !self.first_frame_shown {
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            self.first_frame_shown = true;
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
                let area_response = ui.allocate_response(ui.available_size(), Sense::click_and_drag());

                if let Some(doc) = &self.doc {
                    let scroll_output = ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.horizontal_top(|ui| {
                                ui.add_space(32.0);
                                ui.vertical(|ui| {
                                    let max_width = ui.available_width().min(900.0);
                                    ui.set_max_width(max_width);
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
                                ui.add_space(32.0);
                            });
                        });
                    // Update viewport scroll state for next frame's culling
                    self.viewport.scroll_offset = scroll_output.state.offset.y;
                    self.viewport.viewport_height = scroll_output.inner_rect.height();
                } else if let Some(err) = self.error_msg.clone() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() / 3.0);
                        ui.label(RichText::new("⚠ 无法打开文件").size(24.0).color(egui::Color32::from_rgb(220, 80, 60)));
                        ui.add_space(8.0);
                        ui.label(RichText::new(&err).size(14.0).color(self.theme.muted_text()));
                        ui.add_space(16.0);
                        if ui.button("拖入 .md 文件或从命令行打开").clicked() {
                            self.error_msg = None;
                        }
                    });
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() / 3.0);
                        ui.label(RichText::new("mdview").size(32.0).color(self.theme.muted_text()));
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Drop a .md file here, or open from command line")
                                .size(14.0)
                                .color(self.theme.muted_text()),
                        );
                    });
                }

                // Handle text selection
                self.selector.handle_input(&area_response);
                self.selector.draw_selection(ui, self.theme.selection_bg);
                area_response.context_menu(|ui| {
                    if ui.button("复制文本").clicked() {
                        self.copy_selected_text();
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.menu_button("字体大小", |ui| {
                        for size in [12.0, 14.0, 16.0, 18.0, 20.0] {
                            let label = format!("{}px", size as i32);
                            if self.font_size == size {
                                ui.label(RichText::new(format!("✓ {}", label)).strong());
                            } else if ui.button(&label).clicked() {
                                self.font_size = size;
                                ui.close_menu();
                            }
                        }
                    });
                    ui.menu_button("切换主题", |ui| {
                        for theme in Theme::all_themes() {
                            if std::ptr::eq(theme, self.theme) {
                                ui.label(RichText::new(format!("✓ {}", theme.name)).strong());
                            } else if ui.button(theme.name).clicked() {
                                self.theme = theme;
                                ui.close_menu();
                            }
                        }
                    });
                    ui.separator();
                    if ui.button("打开文件目录").clicked() {
                        self.open_file_directory();
                        ui.close_menu();
                    }
                });
            });
    }
}
