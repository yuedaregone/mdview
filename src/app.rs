use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use egui::*;

use crate::config::AppConfig;
use crate::file_watcher::SimpleFileWatcher;
use crate::font;
use crate::markdown::cache::AstCache;
use crate::markdown::parser::MarkdownDoc;
use crate::selection::TextSelector;
use crate::theme::Theme;
use crate::update;
use crate::viewport::ViewportState;

/// 创建界面前预先准备好的启动数据
pub struct AppBootstrap {
    pub config: AppConfig,
    pub doc: Option<MarkdownDoc>,
    pub file_path: Option<PathBuf>,
    pub theme: Theme,
    pub file_watcher: SimpleFileWatcher,
    pub prepared_fonts: font::PreparedFonts,
}

/// 主应用状态
pub struct MdViewApp {
    /// 当前文件路径
    file_path: Option<PathBuf>,
    /// 解析后的 markdown 文档
    doc: Option<Arc<MarkdownDoc>>,
    /// 当前主题
    theme: Theme,
    /// 已同步到 egui 的主题
    applied_theme: Theme,
    /// 基础字体大小
    font_size: f32,
    /// 文本选择状态
    selector: TextSelector,
    /// 视口裁剪状态
    viewport: ViewportState,
    /// AST 缓存（避免重复解析）
    ast_cache: AstCache,
    /// 错误信息
    error_msg: Option<String>,
    /// 应用配置
    config: AppConfig,
    /// 窗口是否最大化
    window_maximized: bool,
    /// 简单文件监视器
    file_watcher: SimpleFileWatcher,
    /// 配置是否需要保存
    config_needs_save: bool,
    /// 上次保存配置的时间
    last_save_time: Instant,
    /// 下次渲染文档时是否需要把主滚动区重置到顶部
    scroll_to_top_pending: bool,
}

impl MdViewApp {
    pub fn new(cc: &eframe::CreationContext<'_>, bootstrap: AppBootstrap) -> Self {
        let fonts_changed = bootstrap.prepared_fonts.config_changed();
        font::apply_prepared_fonts(&cc.egui_ctx, bootstrap.prepared_fonts);

        let mut style = (*cc.egui_ctx.style()).clone();
        style.animation_time = 0.0;
        style.visuals = if bootstrap.theme.is_dark {
            Visuals::dark()
        } else {
            Visuals::light()
        };
        // 强制同步背景色
        style.visuals.panel_fill = bootstrap.theme.background;
        style.visuals.extreme_bg_color = bootstrap.theme.background;
        cc.egui_ctx.set_style(style);

        cc.egui_ctx
            .options_mut(|opts| opts.line_scroll_speed = 100.0);

        let font_size = bootstrap.config.font_size;
        let window_maximized = bootstrap.config.maximized;
        let doc = bootstrap.doc.map(Arc::new);

        let app = Self {
            file_path: bootstrap.file_path,
            doc,
            theme: bootstrap.theme.clone(),
            applied_theme: bootstrap.theme,
            font_size,
            selector: TextSelector::new(),
            viewport: ViewportState::new(0), // Ensure viewport starts at the top
            ast_cache: AstCache::default(),
            error_msg: None,
            config: bootstrap.config,
            window_maximized,
            file_watcher: bootstrap.file_watcher,
            config_needs_save: fonts_changed,
            last_save_time: Instant::now(),
            scroll_to_top_pending: false,
        };

        // 提前应用主题，防止首帧闪烁
        app.apply_theme(&cc.egui_ctx);

        app
    }

    /// 加载新文件（使用 AST 缓存）
    pub fn load_file(&mut self, path: PathBuf) {
        self.load_file_inner(path, true);
    }

    fn reload_file(&mut self, path: PathBuf) {
        self.load_file_inner(path, false);
    }

    fn load_file_inner(&mut self, path: PathBuf, scroll_to_top: bool) {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                self.error_msg = None;
                self.doc = Some(self.ast_cache.get_or_parse(&path, &content));
                self.viewport.reset(0); // Ensure viewport starts at the top when loading a new file
                self.scroll_to_top_pending |= scroll_to_top;
                self.file_path = Some(path.clone());
                self.file_watcher = SimpleFileWatcher::new(Some(path.clone()));

                self.save_config();
            }
            Err(e) => {
                self.error_msg = Some(format!("无法打开文件: {}", e));
            }
        }
    }

    /// 保存当前配置
    pub fn save_config(&mut self) {
        self.config.theme_name = Some(self.theme.name.to_string());
        self.config.font_size = self.font_size;
        self.config_needs_save = true;
    }

    /// 应用主题到 egui 视觉效果
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
        // 设置表格条纹颜色
        if let Some(stripe) = self.theme.table_stripe_bg {
            visuals.faint_bg_color = stripe;
        }
        ctx.set_visuals(visuals);
    }

    /// 仅在主题变化时同步 egui visuals
    fn sync_theme(&mut self, ctx: &Context) -> bool {
        if self.applied_theme == self.theme {
            return false;
        }

        self.apply_theme(ctx);
        self.applied_theme = self.theme.clone();
        true
    }

    fn window_title(&self) -> String {
        self.file_path
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .map(|name| format!("{name} - mdview"))
            .unwrap_or_else(|| "mdview".to_string())
    }

    fn render_title_bar(&self, ctx: &Context) {
        let title_bar_height = 30.0;
        let is_maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
        let title_bar_bg = self.theme.background;
        let hover_bg = self.theme.code_bg;
        let text_color = self.theme.foreground;

        TopBottomPanel::top("mdview_title_bar")
            .exact_height(title_bar_height)
            .frame(
                Frame::NONE
                    .fill(title_bar_bg)
                    .stroke(Stroke::new(1.0, self.theme.hr_color))
                    .inner_margin(Margin::symmetric(8, 0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new(self.window_title())
                            .size(12.0)
                            .color(text_color),
                    );

                    let controls_width = 108.0;
                    let drag_width = (ui.available_width() - controls_width).max(0.0);
                    let drag_rect =
                        Rect::from_min_size(ui.cursor().min, vec2(drag_width, title_bar_height));
                    let _drag_response = ui.allocate_rect(drag_rect, Sense::hover());

                    let (primary_pressed, double_clicked, pointer_pos) = ctx.input(|input| {
                        (
                            input.pointer.primary_pressed(),
                            input.pointer.button_double_clicked(PointerButton::Primary),
                            input.pointer.interact_pos(),
                        )
                    });
                    let pointer_in_drag_area =
                        pointer_pos.is_some_and(|pos| drag_rect.contains(pos));

                    if double_clicked && pointer_in_drag_area {
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(!is_maximized));
                    } else if primary_pressed && pointer_in_drag_area {
                        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if title_bar_button(ui, "x", text_color, hover_bg).clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                        if title_bar_button(ui, "[]", text_color, hover_bg).clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Maximized(!is_maximized));
                        }
                        if title_bar_button(ui, "_", text_color, hover_bg).clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                        }
                    });
                });
            });
    }
}

fn title_bar_button(ui: &mut Ui, text: &str, text_color: Color32, hover_bg: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(34.0, 22.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), hover_bg);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(12.0),
        text_color,
    );
    response
}

impl eframe::App for MdViewApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // 1. 窗口状态跟踪
        update::handle_window_state(
            ctx,
            &mut self.config,
            &mut self.window_maximized,
            &mut self.config_needs_save,
            &mut self.last_save_time,
        );

        // 2. 清除选择器 segments
        self.selector.clear_segments();

        // 3. 防抖动保存配置
        update::flush_config_save(
            &self.config,
            &mut self.config_needs_save,
            &mut self.last_save_time,
        );

        // 4. 处理快捷键
        update::handle_keyboard_shortcuts(
            ctx,
            &mut self.font_size,
            &mut self.theme,
            &mut self.config,
            &mut self.config_needs_save,
            &self.selector,
            &self.file_path,
        );

        // 5. 在渲染前同步主题，保证快捷键切主题当帧生效
        self.sync_theme(ctx);

        // 6. 处理文件监视器
        if update::check_file_watcher(&mut self.file_watcher) {
            if let Some(path) = self.file_path.clone() {
                self.reload_file(path);
                ctx.request_repaint();
            }
        }

        // 7. 处理拖拽文件
        if let Some(path) = update::check_dropped_files(ctx) {
            self.load_file(path);
        }

        self.render_title_bar(ctx);

        // 8. 渲染 UI
        CentralPanel::default()
            .frame(Frame::NONE.fill(self.theme.background))
            .show(ctx, |ui| {
                // 显示右键主菜单
                let menu_id = egui::Id::new("mdview_context_menu");
                crate::context_menu::show_context_menu(ui, &mut self.selector, &self.file_path);

                // 显示右键子菜单
                crate::context_menu::show_submenus(
                    ctx,
                    &mut self.theme,
                    &mut self.font_size,
                    &mut self.config,
                    &mut self.config_needs_save,
                    menu_id,
                );

                // 检测菜单关闭
                crate::context_menu::check_menu_close(ui, menu_id);

                // 渲染内容
                if let Some(doc) = &self.doc {
                    crate::markdown::renderer::render_doc(
                        ui,
                        doc,
                        &self.theme,
                        self.font_size,
                        &mut self.selector,
                        &mut self.viewport,
                        &mut self.scroll_to_top_pending,
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

        // 菜单内切主题发生在渲染过程中，这里补一次同步用于下一次重绘
        if self.sync_theme(ctx) {
            ctx.request_repaint();
        }
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        let _ = self.config.save();
    }
}
