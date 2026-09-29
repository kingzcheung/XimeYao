//! 候选栏菜单面板（布局与 macOS 版 XimeYi 的 candidate_window.rs 面板对齐）。
//!
//! 本模块只包含面板自身的常量、布局几何、页面模型与绘制；
//! 面板状态（展开/页面/hover）挂在 `ui::CandidateWindow` 上，
//! 鼠标交互在 `ui::RenderedView::wnd_proc` 中借助本模块的几何函数完成，
//! 保证绘制与命中测试共用同一套布局来源。

use std::sync::{Arc, OnceLock};

use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1DeviceContext, D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT, D2D1_DRAW_TEXT_OPTIONS_NONE,
    D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    IDWriteFactory1, IDWriteTextFormat, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_METRICS,
};
use windows_core::{w, HSTRING};

use super::model::CandidateModel;

// ── 布局常量（与 macOS 版 candidate_window.rs 对齐，单位为 DIP）──
/// 候选栏右侧 "⋮" 菜单按钮。
pub(crate) const MENU_BUTTON_SIZE: f32 = 20.0;
pub(crate) const MENU_BUTTON_GAP: f32 = 4.0;
/// 面板区高：布局 = 标题栏 36 + 间距 8 + 4 行条目×32（行距 4）+ 间距 8 + 底部区 32 + 底边距 8。
// 面板高度收紧到内容实际需要：菜单页顶部 10 + 4 行卡片 140 + 间距 8
// + 品牌栏 32 + 底边距 8 = 198（菜单页无标题栏，子页面内容少、此高度足够）。
const PANEL_HEIGHT: f32 = 198.0;
/// 面板区与候选区之间的间距。
const PANEL_GAP: f32 = 4.0;
/// 面板标题栏高度。
const PANEL_HEADER_HEIGHT: f32 = 36.0;
/// 面板行统一高度：菜单卡片 / 底部入口条同高。
const PANEL_ITEM_HEIGHT: f32 = 32.0;
/// 行背景块之间的垂直间距。
const PANEL_ROW_GAP: f32 = 4.0;
/// 标题栏 / 条目行区 / 底部区之间的统一间距。
const PANEL_CONTENT_GAP: f32 = 8.0;
/// 面板底边距。
const PANEL_BOTTOM_MARGIN: f32 = 8.0;
/// 面板菜单列数。
const PANEL_MENU_COLUMNS: usize = 2;
/// 菜单两列卡片之间的水平间距。
const PANEL_MENU_COL_GAP: f32 = 8.0;
/// 面板内容区统一水平边距。
const PANEL_H_INSET: f32 = 10.0;
/// 面板展开时的窗口最小宽度：候选栏本身可能很窄，菜单卡片放不下。
pub(crate) const PANEL_MIN_WIDTH: f32 = 320.0;
/// 面板页面：返回按钮区域宽度/高度（非菜单页显示于标题栏右侧）。
const PANEL_BACK_WIDTH: f32 = 64.0;
const PANEL_BACK_HEIGHT: f32 = 24.0;

/// 面板页面（候选栏下方面板可承载多个页面，菜单页为默认首页）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PanelPage {
    /// 菜单（默认页，2 列功能入口）。
    #[default]
    Menu,
    /// 剪切板。
    Clipboard,
    /// 快捷发送。
    QuickSend,
    /// 计算器。
    Calculator,
    /// 表情。
    Emoji,
    /// 符号。
    Symbol,
    /// 语音输入。
    VoiceInput,
    /// 设置。
    Settings,
}

impl PanelPage {
    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "clipboard" => Self::Clipboard,
            "quick_send" => Self::QuickSend,
            "calculator" => Self::Calculator,
            "emoji" => Self::Emoji,
            "symbol" => Self::Symbol,
            "voice_input" => Self::VoiceInput,
            "settings" => Self::Settings,
            _ => return None,
        })
    }

    fn title(&self) -> &'static str {
        match self {
            Self::Menu => "菜单",
            Self::Clipboard => "剪切板",
            Self::QuickSend => "快捷发送",
            Self::Calculator => "计算器",
            Self::Emoji => "表情",
            Self::Symbol => "符号",
            Self::VoiceInput => "语音输入",
            Self::Settings => "设置",
        }
    }
}

/// 面板菜单项触发的动作（交由外部回调处理）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuAction {
    /// 打开设置程序。
    OpenSettings,
}

static PANEL_ACTION_CALLBACK: OnceLock<Arc<dyn Fn(MenuAction) + Send + Sync>> = OnceLock::new();

/// 设置面板动作回调（例如启动 winxime-setup）。
pub(crate) fn set_panel_action_callback(callback: Arc<dyn Fn(MenuAction) + Send + Sync>) {
    let _ = PANEL_ACTION_CALLBACK.set(callback);
}

/// 触发面板动作回调（未设置回调时忽略）。
pub(crate) fn dispatch_action(action: MenuAction) {
    if let Some(callback) = PANEL_ACTION_CALLBACK.get() {
        callback(action);
    }
}

/// 菜单页顶部留白（面板内坐标）：菜单页无标题栏，内容直接从顶部开始。
const PANEL_MENU_TOP: f32 = 10.0;

/// 菜单页第 i 行（从顶部数）的 y（面板内坐标，y 向下）：行高 32、行距 4。
/// 菜单卡片与底部入口条共用同一套行距节奏。
fn panel_menu_row_y(i: usize) -> f32 {
    PANEL_MENU_TOP + i as f32 * (PANEL_ITEM_HEIGHT + PANEL_ROW_GAP)
}

/// 底部入口条（菜单页品牌栏）的 y。
fn panel_footer_y() -> f32 {
    PANEL_HEIGHT - PANEL_BOTTOM_MARGIN - PANEL_ITEM_HEIGHT
}

/// 面板展开时相对候选栏增加的窗口高度（DIP）。
pub(crate) fn panel_extra_height() -> f32 {
    PANEL_GAP + PANEL_HEIGHT
}

/// 面板页面标题栏右侧的 "← 菜单" 返回按钮矩形 (left, top, right, bottom)。
fn panel_back_rect(width: f32) -> (f32, f32, f32, f32) {
    let x = width - PANEL_BACK_WIDTH - PANEL_H_INSET;
    let y = (PANEL_HEADER_HEIGHT - PANEL_BACK_HEIGHT) / 2.0;
    (x, y, x + PANEL_BACK_WIDTH, y + PANEL_BACK_HEIGHT)
}

/// 矩形 (left, top, right, bottom) 是否包含点。
pub(crate) fn rect_contains(r: (f32, f32, f32, f32), x: f32, y: f32) -> bool {
    x >= r.0 && x <= r.2 && y >= r.1 && y <= r.3
}

/// 面板菜单项（2 列布局）：id 供点击逻辑区分功能。
struct PanelMenuItem {
    id: &'static str,
    icon: &'static str,
    label: &'static str,
    rect: (f32, f32, f32, f32),
}

/// 面板菜单布局：2 列 × 4 行（7 个功能 + 1 空位），图标 + 文字。
/// 菜单项与 macOS 版 MENU_DEFS 一致（后续逐个接入实际功能）：
///     📋 剪切板 / 🚀 快捷发送 / 🧮 计算器 / 😀 表情 / 🔣 符号 / 🎙️ 语音输入 / ⚙️ 设置
fn panel_menu_items(width: f32) -> Vec<PanelMenuItem> {
    const MENU_DEFS: [(&str, &str, &str); 7] = [
        ("clipboard", "📋", "剪切板"),
        ("quick_send", "🚀", "快捷发送"),
        ("calculator", "🧮", "计算器"),
        ("emoji", "😀", "表情"),
        ("symbol", "🔣", "符号"),
        ("voice_input", "🎙️", "语音输入"),
        ("settings", "⚙️", "设置"),
    ];
    let col_count = PANEL_MENU_COLUMNS as f32;
    let col_w = (width - 2.0 * PANEL_H_INSET - (col_count - 1.0) * PANEL_MENU_COL_GAP) / col_count;

    MENU_DEFS
        .iter()
        .enumerate()
        .map(|(i, (id, icon, label))| {
            let col = i as f32 % col_count;
            let row = (i as f32 / col_count).floor() as usize;
            let x = PANEL_H_INSET + col * (col_w + PANEL_MENU_COL_GAP);
            let y = panel_menu_row_y(row);
            PanelMenuItem {
                id,
                icon,
                label,
                rect: (x, y, x + col_w, y + PANEL_ITEM_HEIGHT),
            }
        })
        .collect()
}

/// 菜单页第 index 张卡片对应的功能 id。
pub(crate) fn menu_item_id(index: usize, panel_width: f32) -> Option<&'static str> {
    panel_menu_items(panel_width).get(index).map(|item| item.id)
}

/// 面板内命中的交互元素。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanelHit {
    /// 命中菜单页第 i 张卡片。
    MenuItem(usize),
    /// 命中 "← 菜单" 返回按钮。
    Back,
}

/// 面板命中测试（绘制几何与点击/hover 共用的唯一布局来源）。
/// `x`/`y` 为面板内坐标（相对面板左上角，见 [`window_to_panel`]）。
pub(crate) fn panel_hit(page: PanelPage, panel_width: f32, x: f32, y: f32) -> Option<PanelHit> {
    match page {
        PanelPage::Menu => panel_menu_items(panel_width)
            .iter()
            .enumerate()
            .find(|(_, item)| rect_contains(item.rect, x, y))
            .map(|(i, _)| PanelHit::MenuItem(i)),
        _ => {
            if rect_contains(panel_back_rect(panel_width), x, y) {
                Some(PanelHit::Back)
            } else {
                None
            }
        }
    }
}

/// 把窗口客户区 DIP 坐标换算为面板内坐标；点不在面板区域内时返回 None。
/// `blur_radius` 为窗口四周阴影留白，`bar_height` 为候选栏自身高度。
pub(crate) fn window_to_panel(
    blur_radius: f32,
    bar_height: f32,
    panel_width: f32,
    x: f32,
    y: f32,
) -> Option<(f32, f32)> {
    if panel_width <= 0.0 {
        return None;
    }
    let lx = x - blur_radius;
    let ly = y - (blur_radius + bar_height + PANEL_GAP);
    if lx >= 0.0 && lx <= panel_width && ly >= 0.0 && ly <= PANEL_HEIGHT {
        Some((lx, ly))
    } else {
        None
    }
}

/// 创建文本格式（可指定字号/字重/对齐）。
fn make_text_format(
    dwrite: &IDWriteFactory1,
    family: &HSTRING,
    size: f32,
    weight: DWRITE_FONT_WEIGHT,
    align: DWRITE_TEXT_ALIGNMENT,
    vcenter: bool,
) -> Result<IDWriteTextFormat, String> {
    unsafe {
        let fmt = dwrite
            .CreateTextFormat(
                family,
                None,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!("zh-CN"),
            )
            .map_err(|e| format!("CreateTextFormat failed: {:?}", e))?;
        let _ = fmt.SetTextAlignment(align);
        if vcenter {
            let _ = fmt.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
        }
        Ok(fmt)
    }
}

/// 绘制候选栏下方的菜单面板（布局与 macOS 版一致：标题栏 + 2 列卡片 + 底部品牌栏）。
/// 面板内坐标均为 panel-local，绘制时加上面板原点偏移。
pub(crate) fn draw_panel(
    d2d: &ID2D1DeviceContext,
    dwrite: &IDWriteFactory1,
    model: &CandidateModel,
    blur_radius: f32,
    panel_width: f32,
    bar_height: f32,
    page: PanelPage,
    hovered_menu: Option<usize>,
) -> Result<(), String> {
    unsafe {
        let panel_y = blur_radius + bar_height + PANEL_GAP;
        let panel_left = blur_radius;
        let panel_right = blur_radius + panel_width;
        let radius = 8.0;

        // 面板背景（比候选栏略深的灰调，与 macOS 版一致）。
        let panel_bg_brush = d2d
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 0.92,
                    g: 0.92,
                    b: 0.94,
                    a: 0.96,
                },
                None,
            )
            .map_err(|e| format!("CreateSolidColorBrush panel_bg failed: {:?}", e))?;
        let panel_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: panel_left,
                top: panel_y,
                right: panel_right,
                bottom: panel_y + PANEL_HEIGHT,
            },
            radiusX: radius,
            radiusY: radius,
        };
        d2d.FillRoundedRectangle(&panel_rect, &panel_bg_brush);

        let fg = model.fg_color;
        let text_brush = d2d
            .CreateSolidColorBrush(&fg, None)
            .map_err(|e| format!("CreateSolidColorBrush panel_text failed: {:?}", e))?;
        let secondary_brush = d2d
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: fg.r,
                    g: fg.g,
                    b: fg.b,
                    a: 0.55,
                },
                None,
            )
            .map_err(|e| format!("CreateSolidColorBrush panel_secondary failed: {:?}", e))?;
        let card_brush = d2d
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: fg.r,
                    g: fg.g,
                    b: fg.b,
                    a: 0.06,
                },
                None,
            )
            .map_err(|e| format!("CreateSolidColorBrush panel_card failed: {:?}", e))?;
        let hover_brush = d2d
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: model.highlight_bg_color.r,
                    g: model.highlight_bg_color.g,
                    b: model.highlight_bg_color.b,
                    a: 0.15,
                },
                None,
            )
            .map_err(|e| format!("CreateSolidColorBrush panel_hover failed: {:?}", e))?;
        let line_brush = d2d
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: fg.r,
                    g: fg.g,
                    b: fg.b,
                    a: 0.06,
                },
                None,
            )
            .map_err(|e| format!("CreateSolidColorBrush panel_line failed: {:?}", e))?;

        let center_format = make_text_format(
            dwrite,
            &model.font_family,
            model.font_size,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            true,
        )?;
        let left_format = make_text_format(
            dwrite,
            &model.font_family,
            model.font_size,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            true,
        )?;

        match page {
            PanelPage::Menu => {
                // 菜单页：2 列功能入口卡片（行高/行距与 macOS 版一致），底部品牌栏补齐版面。
                // 卡片图标为 emoji 字符：候选字体（中文）没有 emoji 字形，会渲染成方框；
                // 必须用系统 emoji 字体 + 彩色字形选项（Win10+）。
                let emoji_family = HSTRING::from("Segoe UI Emoji");
                let icon_format = make_text_format(
                    dwrite,
                    &emoji_family,
                    model.font_size + 2.0,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    true,
                )?;
                for (i, item) in panel_menu_items(panel_width).iter().enumerate() {
                    let rect = (
                        panel_left + item.rect.0,
                        panel_y + item.rect.1,
                        panel_left + item.rect.2,
                        panel_y + item.rect.3,
                    );
                    let card_rect = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: rect.0,
                            top: rect.1,
                            right: rect.2,
                            bottom: rect.3,
                        },
                        radiusX: radius,
                        radiusY: radius,
                    };
                    let card_brush = if hovered_menu == Some(i) {
                        &hover_brush
                    } else {
                        &card_brush
                    };
                    d2d.FillRoundedRectangle(&card_rect, card_brush);

                    let icon_hstring = HSTRING::from(item.icon);
                    let mut icon_metrics = DWRITE_TEXT_METRICS::default();
                    dwrite
                        .CreateTextLayout(&icon_hstring, &icon_format, f32::MAX, f32::MAX)
                        .map_err(|e| format!("CreateTextLayout for menu icon failed: {:?}", e))?
                        .GetMetrics(&mut icon_metrics)
                        .map_err(|e| format!("GetMetrics for menu icon failed: {:?}", e))?;

                    let icon_x = rect.0 + 10.0;
                    let icon_w = icon_metrics.widthIncludingTrailingWhitespace;
                    d2d.DrawText(
                        &icon_hstring,
                        &icon_format,
                        &D2D_RECT_F {
                            left: icon_x,
                            top: rect.1,
                            right: icon_x + icon_w,
                            bottom: rect.3,
                        },
                        &text_brush,
                        D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );

                    let label_hstring = HSTRING::from(item.label);
                    d2d.DrawText(
                        &label_hstring,
                        &left_format,
                        &D2D_RECT_F {
                            left: icon_x + icon_w + 8.0,
                            top: rect.1,
                            right: rect.2,
                            bottom: rect.3,
                        },
                        &text_brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                }

                // 品牌栏：占据底部入口条分区（非交互），与 macOS 版一致。
                let brand_hstring = HSTRING::from("曦码·曜输入法");
                let brand_format = make_text_format(
                    dwrite,
                    &model.font_family,
                    model.font_size - 2.0,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    true,
                )?;
                d2d.DrawText(
                    &brand_hstring,
                    &brand_format,
                    &D2D_RECT_F {
                        left: panel_left,
                        top: panel_y + panel_footer_y(),
                        right: panel_right,
                        bottom: panel_y + panel_footer_y() + PANEL_ITEM_HEIGHT,
                    },
                    &secondary_brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
            page => {
                // 其它功能页面（v1 占位）：标题（粗体）+ 返回按钮 + 占位内容，与 macOS 一致。
                let title_format = make_text_format(
                    dwrite,
                    &model.font_family,
                    model.font_size + 1.0,
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    true,
                )?;
                let title_hstring = HSTRING::from(page.title());
                d2d.DrawText(
                    &title_hstring,
                    &title_format,
                    &D2D_RECT_F {
                        left: panel_left + PANEL_H_INSET + 8.0,
                        top: panel_y,
                        right: panel_right,
                        bottom: panel_y + PANEL_HEADER_HEIGHT,
                    },
                    &text_brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );

                // 标题栏细分隔线。
                d2d.FillRectangle(
                    &D2D_RECT_F {
                        left: panel_left + PANEL_H_INSET,
                        top: panel_y + PANEL_HEADER_HEIGHT - 0.5,
                        right: panel_right - PANEL_H_INSET,
                        bottom: panel_y + PANEL_HEADER_HEIGHT + 0.5,
                    },
                    &line_brush,
                );

                // "← 菜单" 返回按钮。
                let back = panel_back_rect(panel_width);
                let back_hstring = HSTRING::from("← 菜单");
                d2d.DrawText(
                    &back_hstring,
                    &left_format,
                    &D2D_RECT_F {
                        left: panel_left + back.0,
                        top: panel_y + back.1,
                        right: panel_left + back.2,
                        bottom: panel_y + back.3,
                    },
                    &secondary_brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );

                // 占位内容：居中于标题栏与品牌栏之间。
                let placeholder_hstring = HSTRING::from("功能开发中");
                let rows_top = PANEL_HEADER_HEIGHT + PANEL_CONTENT_GAP;
                let rows_bottom = panel_footer_y() - PANEL_ROW_GAP;
                let placeholder_cy = (rows_top + rows_bottom) / 2.0;
                d2d.DrawText(
                    &placeholder_hstring,
                    &center_format,
                    &D2D_RECT_F {
                        left: panel_left,
                        top: panel_y + placeholder_cy - PANEL_ITEM_HEIGHT / 2.0,
                        right: panel_right,
                        bottom: panel_y + placeholder_cy + PANEL_ITEM_HEIGHT / 2.0,
                    },
                    &secondary_brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        }

        Ok(())
    }
}

/// 绘制候选栏最右侧的 "⋮" 菜单按钮（仅横向布局）。
/// `rect` 为按钮在候选栏内容坐标系（原点=候选栏内容左上角）中的矩形。
pub(crate) fn draw_menu_button(
    d2d: &ID2D1DeviceContext,
    dwrite: &IDWriteFactory1,
    model: &CandidateModel,
    blur_radius: f32,
    rect: (f32, f32, f32, f32),
) -> Result<(), String> {
    unsafe {
        let format = make_text_format(
            dwrite,
            &model.font_family,
            model.font_size + 2.0,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            true,
        )?;
        let brush = d2d
            .CreateSolidColorBrush(&model.selkey_color, None)
            .map_err(|e| format!("CreateSolidColorBrush menu_btn failed: {:?}", e))?;
        let buf: Vec<u16> = "⋮".encode_utf16().collect();
        d2d.DrawText(
            &buf,
            &format,
            &D2D_RECT_F {
                left: blur_radius + rect.0,
                top: blur_radius + rect.1,
                right: blur_radius + rect.2,
                bottom: blur_radius + rect.3,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_items_layout_two_columns_four_rows() {
        let items = panel_menu_items(PANEL_MIN_WIDTH);
        assert_eq!(items.len(), 7);
        let col_w =
            (PANEL_MIN_WIDTH - 2.0 * PANEL_H_INSET - PANEL_MENU_COL_GAP) / 2.0;
        for (i, item) in items.iter().enumerate() {
            let col = i % PANEL_MENU_COLUMNS;
            let row = i / PANEL_MENU_COLUMNS;
            let expect_x = PANEL_H_INSET + col as f32 * (col_w + PANEL_MENU_COL_GAP);
            assert!((item.rect.0 - expect_x).abs() < 1e-4, "item {i} x");
            assert!((item.rect.1 - panel_menu_row_y(row)).abs() < 1e-4, "item {i} y");
            assert!((item.rect.2 - item.rect.0 - col_w).abs() < 1e-4, "item {i} w");
            assert!(
                (item.rect.3 - item.rect.1 - PANEL_ITEM_HEIGHT).abs() < 1e-4,
                "item {i} h"
            );
        }
    }

    #[test]
    fn menu_rows_fit_above_footer() {
        // 最后一行菜单卡片不得与底部品牌栏重叠。
        let last_row_bottom = panel_menu_row_y(3) + PANEL_ITEM_HEIGHT;
        assert!(last_row_bottom <= panel_footer_y() - PANEL_ROW_GAP);
        // 顶部不留大空白：首行紧贴菜单页顶部留白之下。
        assert!(panel_menu_row_y(0) >= PANEL_MENU_TOP);
        assert!(panel_menu_row_y(0) < PANEL_HEADER_HEIGHT);
    }

    #[test]
    fn back_button_inside_header() {
        let back = panel_back_rect(PANEL_MIN_WIDTH);
        // (left, top, right, bottom)：右/底边不得越界面板，尺寸恰为常量值。
        assert!(back.1 >= 0.0 && back.3 <= PANEL_HEADER_HEIGHT);
        assert!(back.0 >= 0.0 && back.2 <= PANEL_MIN_WIDTH);
        assert!((back.2 - back.0 - PANEL_BACK_WIDTH).abs() < 1e-4);
        assert!((back.3 - back.1 - PANEL_BACK_HEIGHT).abs() < 1e-4);
    }

    #[test]
    fn panel_hit_routes_menu_and_back() {
        let w = PANEL_MIN_WIDTH;
        let items = panel_menu_items(w);
        // 第 7 项（settings）在第 4 行第 1 列，命中其中心。
        let settings = &items[6];
        assert_eq!(
            panel_hit(
                PanelPage::Menu,
                w,
                (settings.rect.0 + settings.rect.2) / 2.0,
                (settings.rect.1 + settings.rect.3) / 2.0,
            ),
            Some(PanelHit::MenuItem(6))
        );
        assert_eq!(menu_item_id(6, w), Some("settings"));
        // 子页面只有返回按钮可命中。
        let back = panel_back_rect(w);
        assert_eq!(
            panel_hit(
                PanelPage::Calculator,
                w,
                (back.0 + back.2) / 2.0,
                (back.1 + back.3) / 2.0,
            ),
            Some(PanelHit::Back)
        );
        // 空白处不命中。
        assert_eq!(panel_hit(PanelPage::Menu, w, 0.5, 0.5), None);
    }

    #[test]
    fn window_to_panel_maps_inside_and_outside() {
        let blur = 8.0;
        let bar_h = 30.0;
        // 候选栏区域内不算面板。
        assert!(
            window_to_panel(blur, bar_h, 320.0, 100.0, blur + bar_h / 2.0).is_none(),
            "bar area must not map to panel"
        );
        // 面板首行内可命中，返回面板内坐标。
        let (lx, ly) = window_to_panel(
            blur,
            bar_h,
            320.0,
            blur + 10.0,
            blur + bar_h + PANEL_GAP + 5.0,
        )
        .unwrap();
        assert!((lx - 10.0).abs() < 1e-4);
        assert!((ly - 5.0).abs() < 1e-4);
        // 面板宽度无效时不命中。
        assert!(window_to_panel(blur, bar_h, 0.0, 100.0, 100.0).is_none());
    }

    #[test]
    fn page_from_id_covers_all_menu_defs() {
        for id in [
            "clipboard",
            "quick_send",
            "calculator",
            "emoji",
            "symbol",
            "voice_input",
            "settings",
        ] {
            assert!(PanelPage::from_id(id).is_some(), "id {id}");
        }
        assert!(PanelPage::from_id("unknown").is_none());
    }
}
