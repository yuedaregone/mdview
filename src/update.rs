//! 每帧更新逻辑
//!
//! 包含窗口状态跟踪、快捷键处理、文件监视轮询、拖拽文件处理等

use std::path::PathBuf;
use std::time::Instant;

use egui::*;

use crate::config::AppConfig;
use crate::file_watcher::SimpleFileWatcher;
use crate::selection::TextSelector;
use crate::theme::Theme;

/// 处理窗口状态变化（最大化、尺寸、位置）并请求保存配置
pub fn handle_window_state(
    ctx: &Context,
    config: &mut AppConfig,
    window_maximized: &mut bool,
    config_needs_save: &mut bool,
    last_save_time: &mut Instant,
) {
    let currently_maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
    let viewport_rect = ctx.input(|i| i.viewport().outer_rect);

    if currently_maximized != *window_maximized {
        *window_maximized = currently_maximized;
        config.maximized = currently_maximized;

        if !currently_maximized {
            if let Some(rect) = viewport_rect {
                config.window_x = Some(rect.min.x);
                config.window_y = Some(rect.min.y);
            }
        }
        request_save(config_needs_save, last_save_time);
    } else if !currently_maximized {
        let size = ctx.available_rect();
        if size.width() > 0.0 && size.height() > 0.0 {
            let size_changed =
                size.width() != config.window_width || size.height() != config.window_height;

            let need_save_pos = config.window_x.is_none() || config.window_y.is_none();
            let pos_changed = if let Some(rect) = viewport_rect {
                config
                    .window_x
                    .is_none_or(|x| (x - rect.min.x).abs() > 1.0)
                    || config
                        .window_y
                        .is_none_or(|y| (y - rect.min.y).abs() > 1.0)
            } else {
                false
            };

            if size_changed || need_save_pos || pos_changed {
                config.window_width = size.width();
                config.window_height = size.height();

                if let Some(rect) = viewport_rect {
                    config.window_x = Some(rect.min.x);
                    config.window_y = Some(rect.min.y);
                }

                request_save(config_needs_save, last_save_time);
            }
        }
    }
}

/// 处理键盘快捷键
pub fn handle_keyboard_shortcuts(
    ctx: &Context,
    font_size: &mut f32,
    theme: &mut Theme,
    config: &mut AppConfig,
    config_needs_save: &mut bool,
    selector: &TextSelector,
    file_path: &Option<PathBuf>,
) {
    let has_selection = selector.has_selection();

    ctx.input(|input| {
        if input.modifiers.ctrl {
            // Ctrl+C - 复制选中文本
            if input.key_pressed(egui::Key::C) && has_selection {
                selector.copy_to_clipboard();
            }
            // Ctrl+= - 增大字体
            if input.key_pressed(egui::Key::Equals) {
                *font_size = (*font_size + 2.0).min(32.0);
                config.font_size = *font_size;
                *config_needs_save = true;
                crate::markdown::highlight::clear_highlight_cache();
            }
            // Ctrl+- - 减小字体
            if input.key_pressed(egui::Key::Minus) {
                *font_size = (*font_size - 2.0).max(8.0);
                config.font_size = *font_size;
                *config_needs_save = true;
                crate::markdown::highlight::clear_highlight_cache();
            }
            // Ctrl+0 - 重置字体
            if input.key_pressed(egui::Key::Num0) {
                *font_size = 16.0;
                config.font_size = *font_size;
                *config_needs_save = true;
            }
            // Ctrl+O - 打开文件目录
            if input.key_pressed(egui::Key::O) {
                if let Some(path) = file_path {
                    if let Some(dir) = path.parent() {
                        let _ = open::that(dir);
                    }
                }
            }
            // Ctrl+T - 切换主题
            if input.key_pressed(egui::Key::T) {
                let themes = Theme::from_config();
                let current_idx = themes.iter().position(|t| t.name == theme.name);
                if let Some(idx) = current_idx {
                    let next_idx = (idx + 1) % themes.len();
                    *theme = themes[next_idx].clone();
                    config.theme_name = Some(theme.name.clone());
                    *config_needs_save = true;
                    crate::markdown::highlight::clear_highlight_cache();
                }
            }
        }
    });
}

/// 检查文件监视器，返回是否需要重新加载
pub fn check_file_watcher(file_watcher: &mut SimpleFileWatcher) -> bool {
    file_watcher.check()
}

/// 检查拖拽文件，返回需要打开的文件路径
pub fn check_dropped_files(ctx: &Context) -> Option<PathBuf> {
    let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
    if let Some(dropped) = dropped_files.first() {
        if let Some(path) = &dropped.path {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "md" | "markdown" | "txt") {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    ctx.send_viewport_cmd(ViewportCommand::Title(format!("{} — mdview", name)));
                }
                return Some(path.clone());
            }
        }
    }
    None
}

/// 执行防抖动的配置保存
pub fn flush_config_save(
    config: &AppConfig,
    config_needs_save: &mut bool,
    last_save_time: &mut Instant,
) {
    if *config_needs_save
        && last_save_time.elapsed() > std::time::Duration::from_secs(1)
    {
        if let Err(e) = config.save() {
            tracing::error!("Config save failed: {}", e);
        }
        *config_needs_save = false;
        *last_save_time = Instant::now();
    }
}

/// 请求保存配置（设置标志位）
fn request_save(config_needs_save: &mut bool, last_save_time: &mut Instant) {
    *config_needs_save = true;
    *last_save_time = Instant::now();
}
