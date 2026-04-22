//! 右键菜单实现

use egui::*;

use crate::config::AppConfig;
use crate::selection::TextSelector;
use crate::theme::Theme;

const MAIN_MENU_WIDTH: f32 = 100.0;
const SUBMENU_WIDTH: f32 = 140.0;
const MENU_ITEM_HEIGHT: f32 = 24.0;

pub fn show_context_menu(
    ui: &mut Ui,
    _theme: &mut Theme,
    _font_size: &mut f32,
    selector: &mut TextSelector,
    file_path: &Option<std::path::PathBuf>,
    _config: &mut AppConfig,
    _config_needs_save: &mut bool,
) -> bool {
    let menu_id = Id::new("mdview_context_menu");

    // 获取右键点击事件
    let clicked_secondary = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Secondary));

    if clicked_secondary {
        // 使用 latest_pos 而不是 hover_pos，确保在任何区域都能获取到位置
        let pos = ui.input(|i| i.pointer.latest_pos()).or_else(|| {
            ui.input(|i| i.pointer.interact_pos())
        }).or_else(|| {
            ui.input(|i| i.pointer.hover_pos())
        });

        if let Some(pos) = pos {
            ui.ctx().memory_mut(|mem| {
                mem.data.insert_temp(menu_id.with("pos"), pos.to_vec2());
                mem.data.insert_temp(menu_id.with("open"), true);
                mem.data.insert_temp(menu_id.with("submenu_open"), 0u32);
                mem.data.insert_temp(menu_id.with("just_opened"), true);  // 标记刚打开
            });
        }
        return true; // 返回 true 表示刚打开
    }

    // 清除刚打开标记
    ui.ctx().memory_mut(|mem| {
        mem.data.insert_temp(menu_id.with("just_opened"), false);
    });

    // 检查是否打开
    let is_open = ui.ctx().memory(|mem| {
        mem.data.get_temp::<bool>(menu_id.with("open")).unwrap_or(false)
    });

    if !is_open {
        return false;
    }

    let pos = ui.ctx().memory(|mem| {
        mem.data.get_temp::<Vec2>(menu_id.with("pos")).unwrap_or_default()
    });

    // 绘制主菜单
    let area_response = Area::new(menu_id.with("main_area"))
        .order(Order::Foreground)
        .fixed_pos(pos.to_pos2())
        .interactable(true)
        .show(ui.ctx(), |ui| {
            ui.set_max_width(MAIN_MENU_WIDTH);
            ui.set_min_width(MAIN_MENU_WIDTH);
            ui.spacing_mut().item_spacing = vec2(0.0, 1.0);

            Frame::NONE
                .fill(ui.visuals().extreme_bg_color)
                .stroke(Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
                .corner_radius(4.0)
                .show(ui, |ui| {
                    main_menu_items(
                        ui,
                        selector,
                        file_path,
                        menu_id,
                    );
                });
        });

    // 存储主菜单 rect
    let main_rect = area_response.response.rect;
    ui.ctx().memory_mut(|mem| {
        mem.data.insert_temp(menu_id.with("main_rect"), main_rect);
    });

    false
}

/// 检测菜单关闭条件（在 show_submenus 之后调用）
pub fn check_menu_close(ui: &Ui, menu_id: Id) {
    let main_rect = ui.ctx().memory(|mem| {
        mem.data.get_temp::<Rect>(menu_id.with("main_rect")).unwrap_or(Rect::ZERO)
    });

    if main_rect == Rect::ZERO {
        return;
    }

    // 如果菜单刚打开，跳过当帧的关闭检测
    let just_opened = ui.ctx().memory(|mem| {
        mem.data.get_temp::<bool>(menu_id.with("just_opened")).unwrap_or(false)
    });

    if just_opened {
        return;
    }

    // 检测关闭条件
    let any_click = ui.input(|i| i.pointer.any_click());
    let click_pos = ui.input(|i| i.pointer.interact_pos());
    let key_escape = ui.input(|i| i.key_pressed(egui::Key::Escape));

    if key_escape {
        close_menu(ui.ctx(), menu_id);
    } else if any_click {
        if let Some(click_pos) = click_pos {
            let in_main = main_rect.contains(click_pos);
            let in_submenu = ui.ctx().memory(|mem| {
                let sub = mem.data.get_temp::<Rect>(menu_id.with("sub_rect"));
                sub.is_some_and(|r| r.contains(click_pos))
            });

            if !in_main && !in_submenu {
                close_menu(ui.ctx(), menu_id);
            }
        }
    }
}

/// 关闭菜单（包括主菜单和子菜单）
fn close_menu(ctx: &Context, menu_id: Id) {
    ctx.memory_mut(|mem| {
        mem.data.insert_temp(menu_id.with("open"), false);
        mem.data.insert_temp(menu_id.with("submenu_open"), 0u32);
    });
}

fn main_menu_items(
    ui: &mut Ui,
    selector: &mut TextSelector,
    file_path: &Option<std::path::PathBuf>,
    menu_id: Id,
) {
    // 复制
    if menu_item(ui, "复制", selector.has_selection()) {
        selector.copy_to_clipboard();
        close_menu(ui.ctx(), menu_id);
        return;
    }

    ui.separator();

    // 字体大小
    if submenu_item(ui, "字体大小", 1, menu_id) {
        ui.ctx().memory_mut(|mem| {
            mem.data.insert_temp(menu_id.with("submenu_open"), 1u32);
        });
    }

    // 切换主题
    if submenu_item(ui, "主题", 2, menu_id) {
        ui.ctx().memory_mut(|mem| {
            mem.data.insert_temp(menu_id.with("submenu_open"), 2u32);
        });
    }

    ui.separator();

    // 打开目录
    if menu_item(ui, "打开目录", file_path.is_some()) {
        if let Some(path) = file_path {
            if let Some(dir) = path.parent() {
                let _ = open::that(dir);
            }
        }
        close_menu(ui.ctx(), menu_id);
        return;
    }
}

fn menu_item(ui: &mut Ui, text: &str, enabled: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(
        vec2(MAIN_MENU_WIDTH, MENU_ITEM_HEIGHT),
        Sense::click(),
    );

    if enabled && response.hovered() {
        ui.painter().rect_filled(rect, 2.0, ui.visuals().widgets.hovered.bg_fill);
    }

    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(13.0),
        if enabled { ui.visuals().text_color() } else { ui.visuals().weak_text_color() },
    );

    response.clicked()
}

fn submenu_item(ui: &mut Ui, text: &str, index: u32, menu_id: Id) -> bool {
    let (rect, response) = ui.allocate_exact_size(
        vec2(MAIN_MENU_WIDTH, MENU_ITEM_HEIGHT),
        Sense::hover(),
    );

    let active_index = ui.ctx().memory(|mem| {
        mem.data.get_temp::<u32>(menu_id.with("submenu_open")).unwrap_or(0)
    });

    if response.hovered() || active_index == index {
        ui.painter().rect_filled(rect, 2.0, ui.visuals().widgets.hovered.bg_fill);
    }

    ui.painter().text(
        pos2(rect.center().x - 4.0, rect.center().y),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(13.0),
        ui.visuals().text_color(),
    );

    ui.painter().text(
        pos2(rect.right() - 10.0, rect.center().y),
        Align2::CENTER_CENTER,
        "▶",
        FontId::proportional(7.0),
        ui.visuals().weak_text_color(),
    );

    response.hovered()
}

/// 绘制子菜单
pub fn show_submenus(
    ctx: &Context,
    theme: &mut Theme,
    font_size: &mut f32,
    config: &mut AppConfig,
    config_needs_save: &mut bool,
    menu_id: Id,
) {
    let submenu_open = ctx.memory(|mem| {
        mem.data.get_temp::<u32>(menu_id.with("submenu_open")).unwrap_or(0)
    });

    if submenu_open == 0 {
        return;
    }

    let main_rect = ctx.memory(|mem| {
        mem.data.get_temp::<Rect>(menu_id.with("main_rect")).unwrap_or(Rect::ZERO)
    });

    if main_rect == Rect::ZERO {
        return;
    }

    let pos = vec2(main_rect.right(), main_rect.top());

    if submenu_open == 1 {
        draw_submenu(ctx, menu_id, menu_id.with("font_submenu_area"), pos.to_pos2(), |ui| {
            ui.set_max_width(SUBMENU_WIDTH);
            ui.set_min_width(SUBMENU_WIDTH);
            ui.spacing_mut().item_spacing = vec2(0.0, 1.0);

            let sizes = [12.0, 14.0, 16.0, 18.0, 20.0, 24.0];
            for size in sizes {
                let is_current = (*font_size - size).abs() < 0.1;
                if check_item(ui, &format!("{}px", size), is_current) {
                    *font_size = size;
                    config.font_size = size;
                    *config_needs_save = true;
                    crate::markdown::highlight::clear_highlight_cache();
                    close_menu(ctx, menu_id);
                }
            }

            ui.separator();

            ui.horizontal(|ui| {
                if ui.small_button("减小 (-2)").clicked() {
                    *font_size = (*font_size - 2.0).max(8.0);
                    config.font_size = *font_size;
                    *config_needs_save = true;
                    crate::markdown::highlight::clear_highlight_cache();
                }
                if ui.small_button("增大 (+2)").clicked() {
                    *font_size = (*font_size + 2.0).min(32.0);
                    config.font_size = *font_size;
                    *config_needs_save = true;
                    crate::markdown::highlight::clear_highlight_cache();
                }
            });

            if small_btn(ui, "重置 (16px)") {
                *font_size = 16.0;
                config.font_size = 16.0;
                *config_needs_save = true;
                crate::markdown::highlight::clear_highlight_cache();
            }
        });
    } else if submenu_open == 2 {
        draw_submenu(ctx, menu_id, menu_id.with("theme_submenu_area"), pos.to_pos2(), |ui| {
            ui.set_max_width(SUBMENU_WIDTH);
            ui.set_min_width(SUBMENU_WIDTH);
            ui.spacing_mut().item_spacing = vec2(0.0, 1.0);

            let themes = Theme::from_config();
            for t in &themes {
                let is_current = t.name == theme.name;
                if check_item(ui, &t.name, is_current) {
                    *theme = t.clone();
                    config.theme_name = Some(t.name.clone());
                    *config_needs_save = true;
                    crate::markdown::highlight::clear_highlight_cache();
                    close_menu(ctx, menu_id);
                }
            }
        });
    }
}

fn draw_submenu(
    ctx: &Context,
    menu_id: Id,
    area_id: Id,
    pos: Pos2,
    content: impl FnOnce(&mut Ui),
) {
    let response = Area::new(area_id)
        .order(Order::Foreground)
        .fixed_pos(pos)
        .interactable(true)
        .show(ctx, |ui| {
            Frame::NONE
                .fill(ctx.style().visuals.extreme_bg_color)
                .stroke(Stroke::new(1.0, ctx.style().visuals.widgets.noninteractive.bg_stroke.color))
                .corner_radius(4.0)
                .show(ui, content);
        })
        .response;

    ctx.memory_mut(|mem| {
        mem.data.insert_temp(menu_id.with("sub_rect"), response.rect);
    });
}

fn check_item(ui: &mut Ui, text: &str, checked: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(
        vec2(SUBMENU_WIDTH, MENU_ITEM_HEIGHT),
        Sense::click(),
    );

    if response.hovered() {
        ui.painter().rect_filled(rect, 2.0, ui.visuals().widgets.hovered.bg_fill);
    }

    // 选中标记方块
    if checked {
        let ind = Rect::from_min_size(
            pos2(rect.left() + 8.0, rect.center().y - 4.0),
            vec2(8.0, 8.0),
        );
        ui.painter().rect_filled(ind, 1.0, ui.visuals().selection.bg_fill);
    }

    ui.painter().text(
        pos2(rect.left() + 24.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(13.0),
        ui.visuals().text_color(),
    );

    response.clicked()
}

fn small_btn(ui: &mut Ui, text: &str) -> bool {
    ui.add(Button::new(text).small()).clicked()
}
