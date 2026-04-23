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

/// 主应用状态
pub struct MdViewApp {
    /// 当前文件路径
    file_path: Option<PathBuf>,
    /// 解析后的 markdown 文档
    doc: Option<Arc<MarkdownDoc>>,
    /// 当前主题
    theme: Theme,
    /// 基础字体大小
    font_size: f32,
    /// 是否已显示首帧
    first_frame_shown: bool,
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
}

impl MdViewApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        doc: Option<MarkdownDoc>,
        file_path: Option<PathBuf>,
    ) -> Self {
        let config = AppConfig::load();

        // 设置字体
        font::setup_fonts(&cc.egui_ctx, &config);

        let font_size = config.font_size;

        // 查找主题
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
}

impl eframe::App for MdViewApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        ctx.options_mut(|opts| opts.line_scroll_speed = 100.0);

        // 1. 窗口状态跟踪
        update::handle_window_state(
            ctx,
            &mut self.config,
            &mut self.window_maximized,
            &mut self.config_needs_save,
            &mut self.last_save_time,
        );

        // 2. 应用主题
        self.apply_theme(ctx);

        // 3. 清除选择器 segments
        self.selector.clear_segments();

        // 4. 防抖动保存配置
        update::flush_config_save(
            &self.config,
            &mut self.config_needs_save,
            &mut self.last_save_time,
        );

        // 5. 标记首帧已显示
        if !self.first_frame_shown {
            self.first_frame_shown = true;
        }

        // 6. 处理快捷键
        update::handle_keyboard_shortcuts(
            ctx,
            &mut self.font_size,
            &mut self.theme,
            &mut self.config,
            &mut self.config_needs_save,
            &self.selector,
            &self.file_path,
        );

        // 7. 处理文件监视器
        if update::check_file_watcher(&mut self.file_watcher) {
            if let Some(path) = self.file_path.clone() {
                self.load_file(path);
                ctx.request_repaint();
            }
        }

        // 8. 处理拖拽文件
        if let Some(path) = update::check_dropped_files(ctx) {
            self.load_file(path);
        }

        // 9. 渲染 UI
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
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        let _ = self.config.save();
    }
}
