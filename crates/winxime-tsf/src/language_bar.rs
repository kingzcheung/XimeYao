use crate::text_input_processor::IpcClientHandle;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tracing::debug;
use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::HICON;
use windows_core::*;

const LANGBAR_ITEM_COOKIE: u32 = 0x42424242;

static mut G_HINST: usize = 0;

pub fn set_instance(hinst: HINSTANCE) {
    unsafe {
        G_HINST = hinst.0 as usize;
    }
}

pub type SharedAsciiMode = Arc<AtomicBool>;
pub type LangBarSinkRef = Arc<Mutex<Option<ITfLangBarItemSink>>>;

#[implement(ITfLangBarItemButton, ITfSource)]
pub struct LangBarItemButton {
    guid: GUID,
    ipc: IpcClientHandle,
    sink_ref: LangBarSinkRef,
    status: std::cell::Cell<u32>,
    ascii_mode: SharedAsciiMode,
}

impl LangBarItemButton {
    pub fn new(
        guid: GUID,
        ipc: IpcClientHandle,
        ascii_mode: SharedAsciiMode,
        sink_ref: LangBarSinkRef,
    ) -> Self {
        Self {
            guid,
            ipc,
            sink_ref,
            status: std::cell::Cell::new(0),
            ascii_mode,
        }
    }
}

fn guid_eq(a: &GUID, b: &GUID) -> bool {
    a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
}

impl ITfLangBarItem_Impl for LangBarItemButton_Impl {
    fn GetInfo(&self, pInfo: *mut TF_LANGBARITEMINFO) -> Result<()> {
        let info = unsafe { &mut *pInfo };
        info.clsidService = crate::class_factory::CLSID_XIME;
        info.guidItem = self.guid;
        info.dwStyle = TF_LBI_STYLE_BTN_BUTTON;
        info.ulSort = 1;
        let desc: Vec<u16> = "曦码·曜\0".encode_utf16().collect();
        for (i, c) in desc.iter().enumerate() {
            if i < 256 {
                info.szDescription[i] = *c;
            }
        }
        Ok(())
    }

    fn GetStatus(&self) -> Result<u32> {
        Ok(self.status.get())
    }

    fn Show(&self, fShow: BOOL) -> Result<()> {
        if fShow.as_bool() {
            self.status.set(self.status.get() & !TF_LBI_STATUS_HIDDEN);
        } else {
            self.status.set(self.status.get() | TF_LBI_STATUS_HIDDEN);
        }
        self.update_sinks(TF_LBI_STATUS)?;
        Ok(())
    }

    fn GetTooltipString(&self) -> Result<BSTR> {
        Ok(BSTR::from("左键切换中/英"))
    }
}

impl LangBarItemButton_Impl {
    fn update_sinks(&self, dwflags: u32) -> Result<()> {
        let sink = match self.sink_ref.try_lock() {
            Ok(g) => g,
            Err(_) => return Ok(()),
        };
        if let Some(ref sink) = *sink {
            unsafe { sink.OnUpdate(dwflags)? };
        }
        Ok(())
    }
}

impl ITfLangBarItemButton_Impl for LangBarItemButton_Impl {
    fn OnClick(&self, click: TfLBIClick, _pt: &POINT, _prcArea: *const RECT) -> Result<()> {
        debug!("LangBarItem: OnClick, click={}", click.0);

        if click == TF_LBI_CLK_RIGHT {
            return Ok(());
        }

        if self.ipc.is_connected() {
            debug!("LangBarItem: calling toggle_ascii_mode");
            let response = self.ipc.toggle_ascii_mode();
            if let Some(response) = response {
                if let Some(status) = response.status {
                    debug!("LangBarItem: ascii_mode={}", status.ascii_mode);
                    self.ascii_mode.store(status.ascii_mode, Ordering::Release);
                    self.update_sinks(TF_LBI_STATUS | TF_LBI_ICON | TF_LBI_TEXT)?;
                }
            }
        }
        Ok(())
    }

    fn InitMenu(&self, _pMenu: Ref<'_, ITfMenu>) -> Result<()> {
        Ok(())
    }

    fn OnMenuSelect(&self, _wID: u32) -> Result<()> {
        Ok(())
    }

    fn GetIcon(&self) -> Result<HICON> {
        Ok(HICON::default())
    }

    fn GetText(&self) -> Result<BSTR> {
        let text = if self.ascii_mode.load(Ordering::Acquire) {
            "英"
        } else {
            "中"
        };
        Ok(BSTR::from(text))
    }
}

impl ITfSource_Impl for LangBarItemButton_Impl {
    fn AdviseSink(&self, riid: *const GUID, punk: Ref<'_, IUnknown>) -> Result<u32> {
        let riid = unsafe { &*riid };

        let expected = GUID::from_values(
            0x1F45381,
            0x0FEB,
            0x4D89,
            [0xBF, 0x8D, 0x89, 0x9D, 0x73, 0x4E, 0x9B, 0x8E],
        );

        if !guid_eq(riid, &expected) {
            return Err(Error::from(HRESULT(0x80040201u32 as i32)));
        }

        let punk_ref = punk
            .as_ref()
            .ok_or_else(|| Error::from(HRESULT(0x80004005u32 as i32)))?;
        let sink: ITfLangBarItemSink = punk_ref.cast()?;

        match self.sink_ref.try_lock() {
            Ok(mut guard) => *guard = Some(sink),
            Err(std::sync::TryLockError::Poisoned(e)) => *e.into_inner() = Some(sink),
            Err(std::sync::TryLockError::WouldBlock) => {}
        }

        Ok(LANGBAR_ITEM_COOKIE)
    }

    fn UnadviseSink(&self, dwCookie: u32) -> Result<()> {
        if dwCookie != LANGBAR_ITEM_COOKIE {
            return Err(Error::from(HRESULT(0x80040200u32 as i32)));
        }
        match self.sink_ref.try_lock() {
            Ok(mut guard) => *guard = None,
            Err(std::sync::TryLockError::Poisoned(e)) => *e.into_inner() = None,
            Err(std::sync::TryLockError::WouldBlock) => {}
        }
        Ok(())
    }
}
