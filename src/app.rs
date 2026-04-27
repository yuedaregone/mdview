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
    /// 无边框窗口的边缘缩放状态
    resize_state: WindowResizeState,
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
        let has_startup_doc = doc.is_some();

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
            scroll_to_top_pending: has_startup_doc,
            resize_state: WindowResizeState::default(),
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

    fn render_title_bar(&self, ctx: &Context, resizing_active: bool) {
        let title_bar_height = 30.0;
        let is_maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
        let title_bar_bg = self.theme.background;
        let hover_bg = self.theme.code_bg;
        let border_color = if is_maximized {
            Color32::TRANSPARENT
        } else {
            self.window_chrome_line_color()
        };
        let text_color = self.theme.foreground;

        TopBottomPanel::top("mdview_title_bar")
            .exact_height(title_bar_height)
            .frame(
                Frame::NONE
                    .fill(title_bar_bg)
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
                    } else if primary_pressed && pointer_in_drag_area && !resizing_active {
                        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if title_bar_button(ui, TitleBarButtonIcon::Close, text_color, hover_bg)
                            .clicked()
                        {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                        let maximize_icon = if is_maximized {
                            TitleBarButtonIcon::Restore
                        } else {
                            TitleBarButtonIcon::Maximize
                        };
                        if title_bar_button(ui, maximize_icon, text_color, hover_bg).clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Maximized(!is_maximized));
                        }
                        if title_bar_button(ui, TitleBarButtonIcon::Minimize, text_color, hover_bg)
                            .clicked()
                        {
                            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                        }
                    });
                });

                if border_color != Color32::TRANSPARENT {
                    let bottom = ui.max_rect().bottom();
                    ui.painter().hline(
                        ui.max_rect().x_range(),
                        bottom - 0.5,
                        Stroke::new(1.0, border_color),
                    );
                }
            });
    }

    fn render_window_border(&self, ctx: &Context) {
        if ctx.input(|input| input.viewport().maximized.unwrap_or(false)) {
            return;
        }

        let rect = ctx.screen_rect().shrink(0.5);
        ctx.layer_painter(LayerId::new(
            Order::Foreground,
            Id::new("mdview_window_border"),
        ))
        .rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, self.window_chrome_line_color()),
            StrokeKind::Inside,
        );
    }

    fn window_chrome_line_color(&self) -> Color32 {
        if self.theme.is_dark {
            Color32::from_rgba_unmultiplied(255, 255, 255, 12)
        } else {
            Color32::from_rgba_unmultiplied(0, 0, 0, 10)
        }
    }
}

#[derive(Clone, Copy)]
enum TitleBarButtonIcon {
    Minimize,
    Maximize,
    Restore,
    Close,
}

#[derive(Debug, Default)]
struct WindowResizeState {
    resizing: bool,
    pending_cursor: Option<CursorIcon>,
}

impl WindowResizeState {
    fn handle(&mut self, ctx: &Context) -> bool {
        if ctx.input(|input| input.viewport().maximized.unwrap_or(false)) {
            self.resizing = false;
            self.pending_cursor = None;
            return false;
        }

        let (pointer_pos, primary_pressed, primary_down) = ctx.input(|input| {
            (
                input.pointer.hover_pos(),
                input.pointer.primary_pressed(),
                input.pointer.primary_down(),
            )
        });

        if self.resizing {
            if !primary_down {
                self.resizing = false;
            }
            return true;
        }

        let Some(pointer_pos) = pointer_pos else {
            self.pending_cursor = None;
            return false;
        };

        let direction = detect_resize_direction(ctx.screen_rect(), pointer_pos);
        if let Some(direction) = direction {
            self.pending_cursor = Some(resize_direction_cursor(direction));
            if primary_pressed {
                ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
                self.resizing = true;
                return true;
            }
        } else {
            self.pending_cursor = None;
        }

        direction.is_some()
    }

    fn apply_cursor(&mut self, ctx: &Context) {
        if let Some(cursor) = self.pending_cursor.take() {
            ctx.set_cursor_icon(cursor);
        }
    }
}

const RESIZE_BORDER_WIDTH: f32 = 5.0;
const RESIZE_CORNER_SIZE: f32 = 10.0;

fn detect_resize_direction(window_rect: Rect, pointer_pos: Pos2) -> Option<ResizeDirection> {
    let min = window_rect.min;
    let max = window_rect.max;

    let near_left = pointer_pos.x <= min.x + RESIZE_BORDER_WIDTH;
    let near_right = pointer_pos.x >= max.x - RESIZE_BORDER_WIDTH;
    let near_top = pointer_pos.y <= min.y + RESIZE_BORDER_WIDTH;
    let near_bottom = pointer_pos.y >= max.y - RESIZE_BORDER_WIDTH;

    let in_left_corner = pointer_pos.x <= min.x + RESIZE_CORNER_SIZE;
    let in_right_corner = pointer_pos.x >= max.x - RESIZE_CORNER_SIZE;
    let in_top_corner = pointer_pos.y <= min.y + RESIZE_CORNER_SIZE;
    let in_bottom_corner = pointer_pos.y >= max.y - RESIZE_CORNER_SIZE;

    if in_top_corner && in_left_corner {
        return Some(ResizeDirection::NorthWest);
    }
    if in_top_corner && in_right_corner {
        return Some(ResizeDirection::NorthEast);
    }
    if in_bottom_corner && in_left_corner {
        return Some(ResizeDirection::SouthWest);
    }
    if in_bottom_corner && in_right_corner {
        return Some(ResizeDirection::SouthEast);
    }

    if near_left {
        return Some(ResizeDirection::West);
    }
    if near_right {
        return Some(ResizeDirection::East);
    }
    if near_top {
        return Some(ResizeDirection::North);
    }
    if near_bottom {
        return Some(ResizeDirection::South);
    }

    None
}

fn resize_direction_cursor(direction: ResizeDirection) -> CursorIcon {
    match direction {
        ResizeDirection::North | ResizeDirection::South => CursorIcon::ResizeVertical,
        ResizeDirection::East | ResizeDirection::West => CursorIcon::ResizeHorizontal,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::ResizeNwSe,
        ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::ResizeNeSw,
    }
}

fn title_bar_button(
    ui: &mut Ui,
    icon: TitleBarButtonIcon,
    text_color: Color32,
    hover_bg: Color32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(34.0, 22.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), hover_bg);
    }
    paint_title_bar_icon(ui.painter(), rect, icon, text_color);
    response
}

fn paint_title_bar_icon(painter: &Painter, rect: Rect, icon: TitleBarButtonIcon, color: Color32) {
    let center = rect.center();
    let stroke = Stroke::new(1.4, color);

    match icon {
        TitleBarButtonIcon::Minimize => {
            painter.line_segment(
                [
                    pos2(center.x - 5.5, center.y + 3.5),
                    pos2(center.x + 5.5, center.y + 3.5),
                ],
                stroke,
            );
        }
        TitleBarButtonIcon::Maximize => {
            let icon_rect = Rect::from_center_size(center, vec2(10.0, 8.0));
            painter.rect_stroke(icon_rect, 0.0, stroke, StrokeKind::Inside);
        }
        TitleBarButtonIcon::Restore => {
            let back = Rect::from_min_size(pos2(center.x - 2.5, center.y - 6.0), vec2(8.0, 6.0));
            let front = Rect::from_min_size(pos2(center.x - 5.0, center.y - 3.0), vec2(8.0, 6.0));
            painter.line_segment([back.left_top(), back.right_top()], stroke);
            painter.line_segment([back.right_top(), back.right_bottom()], stroke);
            painter.rect_stroke(front, 0.0, stroke, StrokeKind::Inside);
        }
        TitleBarButtonIcon::Close => {
            let delta = 4.8;
            painter.line_segment(
                [
                    pos2(center.x - delta, center.y - delta),
                    pos2(center.x + delta, center.y + delta),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(center.x + delta, center.y - delta),
                    pos2(center.x - delta, center.y + delta),
                ],
                stroke,
            );
        }
    }
}

impl eframe::App for MdViewApp {
    fn persist_egui_memory(&self) -> bool {
        false
    }

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

        let resizing_active = self.resize_state.handle(ctx);
        self.render_title_bar(ctx, resizing_active);

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

        self.render_window_border(ctx);
        self.resize_state.apply_cursor(ctx);
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        let _ = self.config.save();
    }
}
