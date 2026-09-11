//! 候选栏窗口模块入口：对外只暴露 `CandidateWindow` 与自定义消息常量。
//!
//! 内部拆分：`model` 数据模型 / `layout` 布局测量 / `paint` D2D 绘制 /
//! `view` 窗口与消息处理 / `panel` 菜单面板。

mod layout;
pub mod panel;
mod model;
mod paint;
mod view;

use std::cell::{Cell, RefCell};
use std::sync::Arc;
use tracing::info;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, PostMessageW, SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, WM_USER,
};
use winxime_ipc::Context;

use self::model::{CandidateModel, RenderedMetrics, RootModel};
use self::panel::PanelPage;
use self::view::RenderedView;

pub const WM_SHOW_CANDIDATE: u32 = WM_USER + 1;
pub const WM_HIDE_CANDIDATE: u32 = WM_USER + 2;
pub const WM_UPDATE_CANDIDATE: u32 = WM_USER + 3;
pub const WM_SET_POSITION: u32 = WM_USER + 4;
pub const WM_SHOW_ROOT: u32 = WM_USER + 5;
pub const WM_HIDE_ROOT: u32 = WM_USER + 6;

/// 共享布局常量（DIP）：layout / paint / view 共用。
pub(crate) const ROW_SPACING: f32 = 4.0;
pub(crate) const COL_SPACING: f32 = 8.0;
pub(crate) const MARGIN: f32 = 6.0;
pub(crate) const MIN_WIDTH: f32 = 120.0;
pub(crate) const BLUR_RADIUS: f32 = 8.0;

pub struct CandidateWindow {
    pub(crate) model: RefCell<CandidateModel>,
    pub(crate) root_model: RefCell<Option<RootModel>>,
    pub(crate) view: RefCell<Option<RenderedView>>,
    /// 面板展开状态（仅 UI 线程 wnd_proc 内读写）。
    pub(crate) panel_visible: Cell<bool>,
    /// 面板当前页面。
    pub(crate) panel_page: Cell<PanelPage>,
    /// 面板菜单页 hover 的卡片下标。
    pub(crate) hovered_menu: Cell<Option<usize>>,
    /// 最近一次布局结果（用于 ⋮ 按钮/面板命中测试）。
    pub(crate) metrics: RefCell<Option<RenderedMetrics>>,
}

unsafe impl Send for CandidateWindow {}
unsafe impl Sync for CandidateWindow {}

impl CandidateWindow {
    /// 当前面板绘制状态（展开时为 Some((页面, hover 卡片下标))）。
    pub(crate) fn panel_paint_state(&self) -> Option<(PanelPage, Option<usize>)> {
        if self.panel_visible.get() {
            Some((self.panel_page.get(), self.hovered_menu.get()))
        } else {
            None
        }
    }

    /// 收起面板并复位到菜单页（输入新内容/隐藏候选栏时调用）。
    pub(crate) fn collapse_panel(&self) {
        self.panel_visible.set(false);
        self.panel_page.set(PanelPage::Menu);
        self.hovered_menu.set(None);
    }

    pub fn new() -> Arc<Self> {
        let window = Arc::new(Self {
            model: RefCell::new(CandidateModel::default()),
            root_model: RefCell::new(None),
            view: RefCell::new(None),
            panel_visible: Cell::new(false),
            panel_page: Cell::new(PanelPage::Menu),
            hovered_menu: Cell::new(None),
            metrics: RefCell::new(None),
        });

        // Initialize UI immediately in the thread that will run message loop
        window.ensure_view_initialized();

        window
    }

    fn ensure_view_initialized(&self) {
        if self.view.borrow().is_none() {
            let user_data_ptr = self as *const Self;
            match RenderedView::new(user_data_ptr.cast()) {
                Ok(view) => {
                    info!("UI initialized successfully");
                    *self.view.borrow_mut() = Some(view);
                }
                Err(e) => {
                    info!("Failed to initialize UI: {}", e);
                }
            }
        }
    }

    pub fn show(&self, x: i32, y: i32) {
        self.ensure_view_initialized();
        if let Some(view) = self.view.borrow().as_ref() {
            info!(
                "  show: hwnd={:?}, moving to ({}, {})",
                view.hwnd.0,
                x,
                y + 24
            );
            unsafe {
                let _ = SetWindowPos(
                    view.hwnd,
                    Some(HWND_TOPMOST),
                    x,
                    y + 24,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOACTIVATE,
                );
                info!("  show: posting WM_SHOW_CANDIDATE");
                let result = PostMessageW(Some(view.hwnd), WM_SHOW_CANDIDATE, WPARAM(0), LPARAM(0));
                info!("  show: PostMessageW result: {:?}", result);
            }
        } else {
            info!("  show: view is None!");
        }
    }

    pub fn hide(&self) {
        if let Some(view) = self.view.borrow().as_ref() {
            unsafe {
                let _ = PostMessageW(Some(view.hwnd), WM_HIDE_CANDIDATE, WPARAM(0), LPARAM(0));
            }
        }
    }

    pub fn update(&self, ctx: &Context) {
        if let Some(view) = self.view.borrow().as_ref() {
            info!(
                "  update: hwnd={:?}, posting WM_UPDATE_CANDIDATE",
                view.hwnd.0
            );
            unsafe {
                let ctx_ptr = Box::into_raw(Box::new(ctx.clone()));
                let result = PostMessageW(
                    Some(view.hwnd),
                    WM_UPDATE_CANDIDATE,
                    WPARAM(ctx_ptr as usize),
                    LPARAM(0),
                );
                info!("  update: PostMessageW result: {:?}", result);
                if result.is_err() {
                    let _ = Box::from_raw(ctx_ptr);
                    info!("  update: PostMessageW failed, freed memory");
                }
            }
        } else {
            info!("  update: view is None!");
        }
    }

    pub fn show_root(&self, letter: char, root: &str) -> Result<(), String> {
        self.ensure_view_initialized();
        if let Some(view) = self.view.borrow().as_ref() {
            let root_model = RootModel::from((letter, root.to_string()));
            *self.root_model.borrow_mut() = Some(root_model.clone());

            unsafe {
                let root_ptr = Box::into_raw(Box::new(root_model));
                let _ = PostMessageW(
                    Some(view.hwnd),
                    WM_SHOW_ROOT,
                    WPARAM(root_ptr as usize),
                    LPARAM(0),
                );
            }
            Ok(())
        } else {
            Err("view is None".to_string())
        }
    }

    pub fn hide_root(&self) {
        if let Some(view) = self.view.borrow().as_ref() {
            *self.root_model.borrow_mut() = None;
            unsafe {
                let _ = PostMessageW(Some(view.hwnd), WM_HIDE_ROOT, WPARAM(0), LPARAM(0));
            }
        }
    }
}
