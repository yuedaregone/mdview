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
    /// 首帧是否已完成渲染
    first_frame_done: bool,
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
            viewport: ViewportState::new(0),
            ast_cache: AstCache::default(),
            error_msg: None,
            config: bootstrap.config,
            window_maximized,
            file_watcher: bootstrap.file_watcher,
            config_needs_save: fonts_changed,
            last_save_time: Instant::now(),
            first_frame_done: false,
        };

        // 提前应用主题，防止首帧闪烁
        app.apply_theme(&cc.egui_ctx);

        app
    }

    /// 加载新文件（使用 AST 缓存）
    pub fn load_file(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                self.error_msg = None;
                self.doc = Some(self.ast_cache.get_or_parse(&path, &content));
                self.viewport.reset(0);
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
                self.load_file(path);
                ctx.request_repaint();
            }
        }

        // 7. 处理拖拽文件
        if let Some(path) = update::check_dropped_files(ctx) {
            self.load_file(path);
        }

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

        // 关键逻辑：首帧渲染后，恢复系统边框和非透明状态
        if !self.first_frame_done {
            ctx.send_viewport_cmd(ViewportCommand::Decorations(true));
            ctx.send_viewport_cmd(ViewportCommand::Transparent(false));
            self.first_frame_done = true;
        }

        // 菜单内切主题发生在渲染过程中，这里补一次同步用于下一次重绘
        if self.sync_theme(ctx) {
            ctx.request_repaint();
        }
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        let _ = self.config.save();
    }
}
