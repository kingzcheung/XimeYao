use librime::{
    get_key_modifiers, vk_to_xk, K_ALT_MASK, K_CONTROL_MASK, K_SHIFT_MASK, VK_BACK, VK_DOWN,
    VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SPACE, VK_TAB,
    VK_UP,
};
use std::sync::Arc;
use tracing::debug;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::TextServices::*;
use windows_core::{Interface, *};
use winxime_ipc::{
    IpcClient, IpcCommand, IpcRequest, IpcRequestData, IpcResponse, KeyEventData, Position,
};

const TF_INVALID_COOKIE: u32 = 0xFFFFFFFF;

const VK_X_A: u16 = 0x41;
const VK_X_Z: u16 = 0x5A;
const VK_X_0: u16 = 0x30;
const VK_X_9: u16 = 0x39;
const VK_OEM_1: u16 = 0xBA;
const VK_OEM_7: u16 = 0xDE;
const VK_OEM_4: u16 = 0xDB;
const VK_OEM_6: u16 = 0xDD;
const VK_OEM_COMMA: u16 = 0xBC;
const VK_OEM_PERIOD: u16 = 0xBE;
const VK_OEM_MINUS: u16 = 0xBD;
const VK_OEM_PLUS: u16 = 0xBB;
const VK_OEM_2: u16 = 0xBF;
const VK_OEM_5: u16 = 0xDC;

const GUID_LBI_INPUTMODE: GUID = GUID::from_u128(0x5D5D8287_5B53_4DAA_B44C_52EB4794A3E7);

struct IpcState {
    client: Option<IpcClient>,
    session_id: u32,
}

pub struct IpcClientHandle {
    state: Arc<std::sync::Mutex<IpcState>>,
}

impl IpcClientHandle {
    pub fn debug_ptr(&self) -> *const () {
        Arc::as_ptr(&self.state) as *const ()
    }

    pub fn new() -> std::result::Result<Self, winxime_ipc::IpcError> {
        let client = IpcClient::connect()?;
        Ok(Self {
            state: Arc::new(std::sync::Mutex::new(IpcState {
                client: Some(client),
                session_id: 0,
            })),
        })
    }

    pub fn empty() -> Self {
        Self {
            state: Arc::new(std::sync::Mutex::new(IpcState {
                client: None,
                session_id: 0,
            })),
        }
    }

    pub fn connect(&self) -> std::result::Result<(), winxime_ipc::IpcError> {
        debug!("IpcClientHandle::connect() called");
        let mut guard = match self.state.try_lock() {
            Ok(g) => g,
            Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                debug!("  -> lock would block, returning error");
                return Err(winxime_ipc::IpcError::ConnectionFailed(
                    "lock would block".to_string(),
                ));
            }
        };
        if guard.client.is_some() {
            debug!("  -> already connected");
            return Ok(());
        }
        debug!("  -> calling IpcClient::connect()");
        let client = IpcClient::connect()?;
        debug!("  -> IpcClient::connect() succeeded");
        guard.client = Some(client);
        Ok(())
    }

    pub fn disconnect(&self) {
        debug!("IpcClientHandle::disconnect() called");
        let mut guard = match self.state.try_lock() {
            Ok(g) => g,
            Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return,
        };
        guard.client = None;
        guard.session_id = 0;
    }

    pub fn is_connected(&self) -> bool {
        match self.state.try_lock() {
            Ok(g) => g.client.is_some(),
            Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner().client.is_some(),
            Err(std::sync::TryLockError::WouldBlock) => false,
        }
    }

    pub fn start_session(&self) -> (u32, Option<IpcResponse>) {
        let mut guard = match self.state.try_lock() {
            Ok(g) => g,
            Err(_) => {
                debug!("start_session: lock failed");
                return (0, None);
            }
        };
        if let Some(ref mut client) = guard.client {
            let request = IpcRequest {
                command: IpcCommand::StartSession,
                session_id: 0,
                data: IpcRequestData::None,
            };
            debug!("start_session: sending request, session_id=0");
            match client.send_request(&request) {
                Ok(response) => {
                    debug!(
                        "start_session: got response, session_id={}, ascii_mode={}",
                        response.session_id,
                        response
                            .status
                            .as_ref()
                            .map(|s| s.ascii_mode)
                            .unwrap_or(false)
                    );
                    guard.session_id = response.session_id;
                    return (guard.session_id, Some(response));
                }
                Err(e) => {
                    debug!("start_session: send_request FAILED: {:?}", e);
                    guard.client = None;
                    guard.session_id = 0;
                }
            }
        } else {
            debug!("start_session: no client");
        }
        (guard.session_id, None)
    }

    pub fn process_key(&self, keycode: i32, modifiers: i32) -> Option<IpcResponse> {
        let mut guard = match self.state.try_lock() {
            Ok(g) => g,
            Err(_) => {
                debug!("process_key: lock failed, returning None");
                return None;
            }
        };
        let session_id = guard.session_id;
        if let Some(ref mut client) = guard.client {
            let request = IpcRequest {
                command: IpcCommand::ProcessKeyEvent,
                session_id,
                data: IpcRequestData::KeyEvent(KeyEventData { keycode, modifiers }),
            };
            match client.send_request(&request) {
                Ok(response) => {
                    debug!("process_key: got response, success={}", response.success);
                    return Some(response);
                }
                Err(e) => {
                    debug!("process_key: send_request FAILED: {:?}", e);
                    guard.client = None;
                    guard.session_id = 0;
                }
            }
        } else {
            debug!("process_key: no client");
        }
        None
    }

    pub fn select_candidate(&self, index: usize) -> Option<IpcResponse> {
        let mut guard = match self.state.try_lock() {
            Ok(g) => g,
            Err(_) => return None,
        };
        let session_id = guard.session_id;
        if let Some(ref mut client) = guard.client {
            let request = IpcRequest {
                command: IpcCommand::SelectCandidate,
                session_id,
                data: IpcRequestData::SelectIndex(index),
            };
            client.send_request(&request).ok()
        } else {
            None
        }
    }

    pub fn change_page(&self, backward: bool) -> Option<IpcResponse> {
        let mut guard = match self.state.try_lock() {
            Ok(g) => g,
            Err(_) => return None,
        };
        let session_id = guard.session_id;
        if let Some(ref mut client) = guard.client {
            let request = IpcRequest {
                command: IpcCommand::ChangePage,
                session_id,
                data: IpcRequestData::ChangePage(backward),
            };
            client.send_request(&request).ok()
        } else {
            None
        }
    }

    pub fn update_position(&self, x: i32, y: i32) {
        let mut guard = match self.state.try_lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let session_id = guard.session_id;
        if let Some(ref mut client) = guard.client {
            let request = IpcRequest {
                command: IpcCommand::UpdateInputPosition,
                session_id,
                data: IpcRequestData::Position(Position { x, y }),
            };
            let _ = client.send_oneway(&request);
        }
    }

    pub fn toggle_ascii_mode(&self) -> Option<IpcResponse> {
        let mut guard = match self.state.try_lock() {
            Ok(g) => g,
            Err(_) => return None,
        };
        let session_id = guard.session_id;
        if let Some(ref mut client) = guard.client {
            let request = IpcRequest {
                command: IpcCommand::ToggleAsciiMode,
                session_id,
                data: IpcRequestData::None,
            };
            client.send_request(&request).ok()
        } else {
            None
        }
    }

    pub fn focus_in(&self) {
        debug!("IPC::focus_in() called");
        let mut guard = match self.state.try_lock() {
            Ok(g) => g,
            Err(_) => {
                debug!("  -> lock failed, skipping focus_in");
                return;
            }
        };
        let session_id = guard.session_id;
        if let Some(ref mut client) = guard.client {
            let request = IpcRequest {
                command: IpcCommand::FocusIn,
                session_id,
                data: IpcRequestData::None,
            };
            debug!("  -> sending FocusIn request (session_id={})", session_id);
            if client.send_oneway(&request).is_ok() {
                debug!("  -> FocusIn sent successfully");
            } else {
                debug!("  -> FocusIn send FAILED");
            }
        } else {
            debug!("  -> no client, cannot send FocusIn");
        }
    }

    pub fn focus_out(&self) {
        debug!("IPC::focus_out() called");
        let mut guard = match self.state.try_lock() {
            Ok(g) => g,
            Err(_) => {
                debug!("  -> lock failed, skipping focus_out");
                return;
            }
        };
        let session_id = guard.session_id;
        if let Some(ref mut client) = guard.client {
            let request = IpcRequest {
                command: IpcCommand::FocusOut,
                session_id,
                data: IpcRequestData::None,
            };
            debug!("  -> sending FocusOut request (session_id={})", session_id);
            if client.send_oneway(&request).is_ok() {
                debug!("  -> FocusOut sent successfully");
            } else {
                debug!("  -> FocusOut send FAILED");
            }
        } else {
            debug!("  -> no client, cannot send FocusOut");
        }
    }

    pub fn show_tray_icon(&self) {
        debug!("IPC::show_tray_icon() called");
        let mut guard = match self.state.try_lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if let Some(ref mut client) = guard.client {
            let request = IpcRequest {
                command: IpcCommand::ShowTrayIcon,
                session_id: 0,
                data: IpcRequestData::None,
            };
            debug!("  -> sending ShowTrayIcon request");
            if client.send_oneway(&request).is_ok() {
                debug!("  -> ShowTrayIcon sent successfully");
            } else {
                debug!("  -> ShowTrayIcon send FAILED");
            }
        } else {
            debug!("  -> no client");
        }
    }

    pub fn hide_tray_icon(&self) {
        debug!("IPC::hide_tray_icon() called");
        let mut guard = match self.state.try_lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if let Some(ref mut client) = guard.client {
            let request = IpcRequest {
                command: IpcCommand::HideTrayIcon,
                session_id: 0,
                data: IpcRequestData::None,
            };
            debug!("  -> sending HideTrayIcon request");
            if client.send_oneway(&request).is_ok() {
                debug!("  -> HideTrayIcon sent successfully");
            } else {
                debug!("  -> HideTrayIcon send FAILED");
            }
        } else {
            debug!("  -> no client");
        }
    }

    pub fn hide_candidates(&self) {
        debug!("IPC::hide_candidates() called");
        let mut guard = match self.state.try_lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let session_id = guard.session_id;
        if let Some(ref mut client) = guard.client {
            let request = IpcRequest {
                command: IpcCommand::HideCandidates,
                session_id,
                data: IpcRequestData::None,
            };
            debug!(
                "  -> sending HideCandidates request (session_id={})",
                session_id
            );
            if client.send_oneway(&request).is_ok() {
                debug!("  -> HideCandidates sent successfully");
            } else {
                debug!("  -> HideCandidates send FAILED");
            }
        } else {
            debug!("  -> no client");
        }
    }

    pub fn show_root(&self, letter: char) {
        debug!("IPC::show_root() called, letter={}", letter);
        let mut guard = match self.state.try_lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let session_id = guard.session_id;
        if let Some(ref mut client) = guard.client {
            let request = IpcRequest {
                command: IpcCommand::ShowRoot,
                session_id,
                data: IpcRequestData::ShowRoot(letter),
            };
            debug!("  -> sending ShowRoot request");
            if client.send_oneway(&request).is_ok() {
                debug!("  -> ShowRoot sent successfully");
            } else {
                debug!("  -> ShowRoot send FAILED");
            }
        } else {
            debug!("  -> no client");
        }
    }

    pub fn hide_root(&self) {
        debug!("IPC::hide_root() called");
        let mut guard = match self.state.try_lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let session_id = guard.session_id;
        if let Some(ref mut client) = guard.client {
            let request = IpcRequest {
                command: IpcCommand::HideRoot,
                session_id,
                data: IpcRequestData::None,
            };
            debug!("  -> sending HideRoot request");
            if client.send_oneway(&request).is_ok() {
                debug!("  -> HideRoot sent successfully");
            } else {
                debug!("  -> HideRoot send FAILED");
            }
        } else {
            debug!("  -> no client");
        }
    }
}

impl Clone for IpcClientHandle {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct RimeOutput {
    commit: Option<String>,
    preedit: String,
    _candidates: Vec<String>,
    composing: bool,
}

impl RimeOutput {
    fn from_response(response: &IpcResponse) -> Self {
        let ctx = response.context.as_ref();
        let commit = ctx.and_then(|c| c.commit.clone());
        debug!(
            "RimeOutput::from_response: context={}, commit={:?}, preedit='{}'",
            ctx.is_some(),
            commit,
            ctx.map(|c| c.preedit.str.clone()).unwrap_or_default()
        );
        Self {
            commit,
            preedit: ctx.map(|c| c.preedit.str.clone()).unwrap_or_default(),
            _candidates: ctx
                .map(|c| c.candidates.candies.iter().map(|t| t.str.clone()).collect())
                .unwrap_or_default(),
            composing: response
                .status
                .as_ref()
                .map(|s| s.composing)
                .unwrap_or(false),
        }
    }
}

#[implement(ITfEditSession)]
struct XimeEditSession {
    output: RimeOutput,
    thread_mgr: Option<ITfThreadMgr>,
    composition: Arc<std::sync::Mutex<Option<ITfComposition>>>,
    composition_sink: ITfCompositionSink,
    ipc: IpcClientHandle,
}

#[implement(ITfCompositionSink)]
struct CompositionSink {
    composition: Arc<std::sync::Mutex<Option<ITfComposition>>>,
}

impl ITfCompositionSink_Impl for CompositionSink_Impl {
    fn OnCompositionTerminated(
        &self,
        _ecwrite: u32,
        _pcomposition: Ref<'_, ITfComposition>,
    ) -> Result<()> {
        debug!("OnCompositionTerminated: composition terminated");
        match self.composition.try_lock() {
            Ok(mut guard) => *guard = None,
            Err(std::sync::TryLockError::Poisoned(e)) => *e.into_inner() = None,
            Err(std::sync::TryLockError::WouldBlock) => {}
        }
        Ok(())
    }
}

impl ITfEditSession_Impl for XimeEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        debug!(
            "DoEditSession: ec={}, commit={}, composing={}, preedit='{}'",
            ec,
            self.output.commit.is_some(),
            self.output.composing,
            self.output.preedit
        );

        let doc_mgr = match self.thread_mgr.as_ref() {
            Some(t) => unsafe { t.GetFocus() }?,
            None => {
                debug!("DoEditSession: no thread_mgr");
                return Ok(());
            }
        };
        let context = unsafe { doc_mgr.GetBase() }?;
        debug!("DoEditSession: got context");

        // Handle commit and composition (weasel pattern)
        let commit_text = self.output.commit.clone().unwrap_or_default();
        let preedit_text = self.output.preedit.clone();

        if !commit_text.is_empty() {
            debug!("DoEditSession: COMMIT '{}'", commit_text);

            // If not composing, start a composition for the commit
            let has_comp = match self.composition.try_lock() {
                Ok(g) => g.is_some(),
                Err(_) => false,
            };
            if !has_comp {
                debug!("DoEditSession: creating composition for commit");
                self.start_composition(&context, ec, "");
            }

            // Set commit text and end composition (clear=false to keep text)
            self.update_composition_text(&context, ec, &commit_text);
            self.end_composition(ec);
            debug!("DoEditSession: commit done");
        }

        // Handle composing state changes and inline preedit update
        let has_comp = match self.composition.try_lock() {
            Ok(g) => g.is_some(),
            Err(_) => false,
        };
        if self.output.composing && !has_comp {
            debug!("DoEditSession: start composition '{}'", preedit_text);
            self.start_composition(&context, ec, &preedit_text);
        } else if self.output.composing && !preedit_text.is_empty() {
            debug!(
                "DoEditSession: update composition text '{}' (has_comp={})",
                preedit_text, has_comp
            );
            self.update_composition_text(&context, ec, &preedit_text);
        } else if !self.output.composing && has_comp {
            debug!("DoEditSession: clear text and end composition");
            self.update_composition_text(&context, ec, "");
            self.end_composition(ec);
        }

        Ok(())
    }
}

impl XimeEditSession_Impl {
    fn update_caret_position_in_session(context: &ITfContext, ec: u32, ipc: &IpcClientHandle) {
        use std::mem::ManuallyDrop;
        use std::ops::Deref;

        unsafe {
            let mut selection = [TF_SELECTION::default(); 1];
            let mut selection_len = 0;
            if context
                .GetSelection(ec, TF_DEFAULT_SELECTION, &mut selection, &mut selection_len)
                .is_ok()
            {
                if let Some(sel_range) = selection[0].range.deref() {
                    if let Ok(view) = context.GetActiveView() {
                        let mut rc = RECT::default();
                        let mut clipped = BOOL::default();
                        if view
                            .GetTextExt(ec, sel_range, &mut rc, &mut clipped)
                            .is_ok()
                        {
                            debug!(
                                "update_caret_position_in_session: left={}, bottom={}",
                                rc.left, rc.bottom
                            );
                            let _ = ipc.update_position(rc.left, rc.bottom);
                        }
                    }
                }
                let [TF_SELECTION { range, .. }] = selection;
                ManuallyDrop::into_inner(range);
            }
        }
    }

    fn start_composition(&self, context: &ITfContext, ec: u32, preedit: &str) {
        debug!("start_composition: preedit='{}'", preedit);
        use windows::Win32::UI::TextServices::{ITfInsertAtSelection, TF_IAS_QUERYONLY};

        let insert_at_selection: ITfInsertAtSelection = match context.cast() {
            Ok(s) => s,
            Err(e) => {
                debug!(
                    "start_composition: cast ITfInsertAtSelection failed: {:?}",
                    e
                );
                return;
            }
        };

        let ctx_comp: ITfContextComposition = match context.cast() {
            Ok(c) => c,
            Err(e) => {
                debug!(
                    "start_composition: cast ITfContextComposition failed: {:?}",
                    e
                );
                return;
            }
        };

        unsafe {
            let range = match insert_at_selection.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[]) {
                Ok(r) => r,
                Err(e) => {
                    debug!("start_composition: InsertTextAtSelection failed: {:?}", e);
                    return;
                }
            };
            debug!("start_composition: got empty range");

            match ctx_comp.StartComposition(ec, &range, Some(&self.composition_sink)) {
                Ok(comp) => {
                    debug!("start_composition: StartComposition succeeded");
                    let comp_range = match comp.GetRange() {
                        Ok(r) => r,
                        Err(e) => {
                            debug!("start_composition: GetRange failed: {:?}", e);
                            return;
                        }
                    };

                    let wide: Vec<u16> = preedit.encode_utf16().collect();
                    debug!("start_composition: setting text {} chars", wide.len());
                    match comp_range.SetText(ec, 0, &wide) {
                        Ok(_) => {
                            debug!("start_composition: SetText succeeded, weasel mode");
                            // Weasel: collapse to end, set selection with TF_AE_NONE
                            comp_range.Collapse(ec, TF_ANCHOR_END).ok();
                            use std::mem::ManuallyDrop;
                            let mut sel = TF_SELECTION::default();
                            sel.range = ManuallyDrop::new(Some(comp_range));
                            sel.style.ase = TF_AE_NONE;
                            sel.style.fInterimChar = FALSE;
                            context.SetSelection(ec, &[sel]).ok();

                            match self.composition.try_lock() {
                                Ok(mut guard) => *guard = Some(comp),
                                Err(std::sync::TryLockError::Poisoned(e)) => {
                                    *e.into_inner() = Some(comp)
                                }
                                Err(std::sync::TryLockError::WouldBlock) => {
                                    // End the composition we just created since we can't track it
                                    comp.EndComposition(ec).ok();
                                }
                            }

                            Self::update_caret_position_in_session(context, ec, &self.ipc);
                        }
                        Err(e) => {
                            debug!("start_composition: SetText failed: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    debug!("start_composition: StartComposition failed: {:?}", e);
                }
            }
        }
    }

    fn update_composition_text(&self, context: &ITfContext, ec: u32, preedit: &str) {
        debug!("update_composition_text: preedit='{}'", preedit);
        let comp = match self.composition.try_lock() {
            Ok(g) => g,
            Err(_) => {
                debug!("update_composition_text: lock failed");
                return;
            }
        };
        if let Some(ref composition) = *comp {
            unsafe {
                if let Ok(range) = composition.GetRange() {
                    let wide: Vec<u16> = preedit.encode_utf16().collect();
                    match range.SetText(ec, 0, &wide) {
                        Ok(_) => {
                            debug!("update_composition_text: SetText succeeded");
                            // Weasel: collapse to end, set selection with TF_AE_NONE
                            range.Collapse(ec, TF_ANCHOR_END).ok();
                            use std::mem::ManuallyDrop;
                            let mut sel = TF_SELECTION::default();
                            sel.range = ManuallyDrop::new(Some(range));
                            sel.style.ase = TF_AE_NONE;
                            sel.style.fInterimChar = FALSE;
                            context.SetSelection(ec, &[sel]).ok();
                        }
                        Err(e) => debug!("update_composition_text: SetText failed: {:?}", e),
                    }
                } else {
                    debug!("update_composition_text: GetRange failed");
                }
            }
        } else {
            debug!("update_composition_text: no composition");
        }
    }

    fn end_composition(&self, ec: u32) {
        match self.composition.try_lock() {
            Ok(mut guard) => {
                if let Some(comp) = guard.take() {
                    unsafe {
                        comp.EndComposition(ec).ok();
                    }
                }
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                if let Some(comp) = e.into_inner().take() {
                    unsafe {
                        comp.EndComposition(ec).ok();
                    }
                }
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                // Lock held by another thread - skip to avoid deadlock
            }
        }
    }
}

impl XimeTextService_Impl {
    fn show_tray_icon_guarded(&self) {
        if !self.tray_visible.get() {
            self.tray_visible.set(true);
            self.ipc.show_tray_icon();
        }
    }

    fn hide_tray_icon_guarded(&self) {
        if self.tray_visible.get() {
            self.tray_visible.set(false);
            self.ipc.hide_tray_icon();
        }
    }

    fn is_composing(&self) -> bool {
        self.composing.get()
    }

    fn set_composing(&self, val: bool) {
        self.composing.set(val);
    }

    fn should_handle_key(&self, vk: VIRTUAL_KEY) -> bool {
        let code = vk.0;

        if self.is_composing() {
            debug!("should_handle_key: composing=true, handle {}", code);
            return true;
        }

        let is_ascii = self.ascii_mode.load(std::sync::atomic::Ordering::Acquire);
        debug!("should_handle_key: code={}, is_ascii={}", code, is_ascii);
        if is_ascii {
            debug!("  -> ascii mode, not handling");
            return false;
        }

        if (VK_X_A..=VK_X_Z).contains(&code) {
            return true;
        }
        if code == VK_RETURN || code == VK_BACK || code == VK_ESCAPE || code == VK_TAB {
            return true;
        }
        if code == VK_SPACE {
            return true;
        }
        if (VK_X_0..=VK_X_9).contains(&code) {
            return true;
        }
        if code == VK_OEM_1 || code == VK_OEM_7 {
            return true;
        }
        if code == VK_OEM_COMMA || code == VK_OEM_PERIOD {
            return true;
        }
        if code == VK_OEM_MINUS || code == VK_OEM_PLUS {
            return true;
        }
        if code == VK_OEM_4 || code == VK_OEM_6 {
            return true;
        }
        if code == VK_OEM_2 || code == VK_OEM_5 {
            return true;
        }
        if code == VK_PRIOR || code == VK_NEXT || code == VK_HOME || code == VK_END {
            return true;
        }
        if code == VK_LEFT || code == VK_RIGHT || code == VK_UP || code == VK_DOWN {
            return true;
        }
        false
    }

    fn update_lang_bar(&self) {
        let sink = match self.lang_bar_sink_ref.try_lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if let Some(ref sink) = *sink {
            unsafe {
                let _ = sink.OnUpdate(TF_LBI_STATUS | TF_LBI_ICON | TF_LBI_TEXT);
            }
        }
    }

    fn update_caret_position_sync(&self, context: &ITfContext) {
        debug!("update_caret_position_sync: getting caret rect");

        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::TextServices::{
            ITfEditSession, TF_DEFAULT_SELECTION, TF_ES_ASYNCDONTCARE, TF_ES_READ,
        };

        #[implement(ITfEditSession)]
        struct SelectionRect {
            context: ITfContext,
            rect: std::cell::Cell<RECT>,
        }

        impl SelectionRect {
            fn new(context: ITfContext) -> Self {
                Self {
                    context,
                    rect: std::cell::Cell::default(),
                }
            }
            fn rect(&self) -> RECT {
                self.rect.get()
            }
        }

        impl ITfEditSession_Impl for SelectionRect_Impl {
            fn DoEditSession(&self, ec: u32) -> Result<()> {
                use std::mem::ManuallyDrop;
                use std::ops::Deref;

                unsafe {
                    let mut selection = [TF_SELECTION::default(); 1];
                    let mut selection_len = 0;
                    self.context.GetSelection(
                        ec,
                        TF_DEFAULT_SELECTION,
                        &mut selection,
                        &mut selection_len,
                    )?;

                    if let Some(sel_range) = selection[0].range.deref() {
                        let view = self.context.GetActiveView()?;
                        let mut rc = RECT::default();
                        let mut clipped = BOOL::default();
                        view.GetTextExt(ec, sel_range, &mut rc, &mut clipped)?;
                        debug!(
                            "SelectionRect: left={}, top={}, right={}, bottom={}",
                            rc.left, rc.top, rc.right, rc.bottom
                        );
                        self.rect.set(rc);
                    }

                    let [TF_SELECTION { range, .. }] = selection;
                    ManuallyDrop::into_inner(range);
                }
                Ok(())
            }
        }

        let tid = self.client_id.get();
        let session = SelectionRect::new(context.clone()).into_object();

        let _ = unsafe {
            context.RequestEditSession(
                tid,
                session.as_interface(),
                TF_ES_ASYNCDONTCARE | TF_ES_READ,
            )
        };

        let rect = session.rect();
        debug!(
            "update_caret_position_sync: left={}, bottom={}",
            rect.left, rect.bottom
        );

        if self.ipc.is_connected() {
            let _ = self.ipc.update_position(rect.left, rect.bottom);
            debug!("update_caret_position_sync: sent to server");
        }
    }

    fn schedule_edit_session(&self, context: &ITfContext, output: RimeOutput) {
        debug!("schedule_edit_session: called");
        let tid = self.client_id.get();
        let mgr = self.thread_mgr.borrow().clone();

        let composition_sink_impl = CompositionSink {
            composition: self.composition.clone(),
        };
        let composition_sink: ITfCompositionSink = composition_sink_impl.into();

        let session = XimeEditSession {
            output,
            thread_mgr: mgr,
            composition: self.composition.clone(),
            composition_sink,
            ipc: self.ipc.clone(),
        };
        let session_itf: ITfEditSession = session.into();
        debug!(
            "schedule_edit_session: tid={}, calling RequestEditSession",
            tid
        );
        unsafe {
            let result = context.RequestEditSession(
                tid,
                &session_itf,
                TF_ES_ASYNCDONTCARE | TF_ES_READWRITE,
            );
            debug!(
                "schedule_edit_session: RequestEditSession result: {:?}",
                result
            );
        }
    }

    fn handle_key_event(&self, context: Option<&ITfContext>, vk: VIRTUAL_KEY) -> bool {
        debug!("handle_key_event: vk={}", vk.0);

        if !self.ipc.is_connected() {
            debug!("  -> IPC not connected, attempting reconnect...");
            if self.ipc.connect().is_ok() {
                self.ipc.start_session();
                debug!("  -> Reconnected!");
            } else {
                debug!("  -> Reconnect failed");
                return false;
            }
        }

        debug!("  -> IPC connected");
        let code = vk.0;
        let mods = get_key_modifiers(false);

        if let Some(ctx) = context {
            self.update_caret_position_sync(ctx);
        }

        if (VK_X_A..=VK_X_Z).contains(&code) && mods == 0 {
            let letter = char::from_u32(code as u32 - VK_X_A as u32 + 'a' as u32).unwrap_or('a');
            self.last_input_key.set(Some(letter));
            debug!("  -> recorded last_input_key: {}", letter);
        }

        let xk = vk_to_xk(code);
        debug!("  -> calling process_key({}, {})", xk, mods);
        let response = self.ipc.process_key(xk, mods);
        debug!("  -> response: {:?}", response);
        if let Some(response) = response {
            debug!("  -> success={}", response.success);
            if response.success {
                let output = RimeOutput::from_response(&response);
                self.set_composing(output.composing);
                debug!(
                    "  -> context is {}",
                    if context.is_some() { "Some" } else { "None" }
                );
                if let Some(ctx) = context {
                    debug!("  -> calling schedule_edit_session");
                    self.schedule_edit_session(ctx, output);
                    debug!("  -> schedule_edit_session returned");
                }
                return true;
            }
        }
        debug!("  -> returning false");
        false
    }

    fn handle_key_up_event(&self, context: Option<&ITfContext>, vk: VIRTUAL_KEY) -> bool {
        debug!("handle_key_up_event: vk={}", vk.0);

        if !self.ipc.is_connected() {
            debug!("  -> IPC not connected, skipping key up");
            return false;
        }

        let code = vk.0;
        let mods = get_key_modifiers(true);

        let xk = vk_to_xk(code);
        debug!("  -> calling process_key({}, {})", xk, mods);
        let response = self.ipc.process_key(xk, mods);
        debug!("  -> response: {:?}", response);
        if let Some(response) = response {
            debug!("  -> success={}", response.success);
            if response.success {
                let output = RimeOutput::from_response(&response);
                self.set_composing(output.composing);
                if let Some(ctx) = context {
                    self.schedule_edit_session(ctx, output);
                }
                return true;
            }
        }
        debug!("  -> returning false");
        false
    }

    fn abort_composition(&self) {
        debug!("abort_composition");
        self.composing.set(false);
        // Release composition reference if held (OnCompositionTerminated will clean up TSF side)
        if let Ok(mut guard) = self.composition.try_lock() {
            *guard = None;
        }
    }
}

impl ITfKeyEventSink_Impl for XimeTextService_Impl {
    fn OnSetFocus(&self, _fforeground: BOOL) -> Result<()> {
        // Focus handling is done in ITfThreadMgrEventSink::OnSetFocus
        // to avoid redundant IPC calls from multiple TSF sinks.
        debug!("ITfKeyEventSink::OnSetFocus - ignored (handled by ThreadMgrEventSink)");
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        _pic: Ref<'_, ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let vk = VIRTUAL_KEY(wparam.0 as u16);
        debug!("OnTestKeyDown: vk={}", vk.0);
        let handled = self.should_handle_key(vk);
        debug!("  -> should_handle_key: {}", handled);
        Ok(BOOL(if handled { 1 } else { 0 }))
    }

    fn OnKeyDown(&self, pic: Ref<'_, ITfContext>, wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        let vk = VIRTUAL_KEY(wparam.0 as u16);
        debug!("OnKeyDown: vk={}", vk.0);

        if vk.0 == VK_CONTROL.0 {
            if self.is_composing() && !self.ctrl_root_visible.get() {
                let last_key = self.last_input_key.get();
                debug!("  -> Ctrl pressed while composing, last_key={:?}", last_key);
                if let Some(letter) = last_key {
                    let mods = get_key_modifiers(false);
                    debug!("  -> mods={}", mods);
                    let ctrl_only = (mods & K_CONTROL_MASK as i32) != 0
                        && (mods & K_SHIFT_MASK as i32) == 0
                        && (mods & K_ALT_MASK as i32) == 0;
                    if ctrl_only {
                        debug!("  -> sending ShowRoot for '{}'", letter);
                        self.ipc.show_root(letter);
                        self.ctrl_root_visible.set(true);
                    }
                }
            }
            return Ok(BOOL(0));
        }

        if !self.should_handle_key(vk) {
            debug!("  -> not handling");
            return Ok(BOOL(0));
        }

        let context = pic.as_ref();
        let handled = self.handle_key_event(context, vk);
        debug!("  -> result: {}", handled);
        Ok(BOOL(if handled { 1 } else { 0 }))
    }

    fn OnTestKeyUp(
        &self,
        _pic: Ref<'_, ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let vk = VIRTUAL_KEY(wparam.0 as u16);
        if vk.0 == VK_SHIFT.0 {
            return Ok(BOOL(1));
        }
        if vk.0 == VK_CONTROL.0 && self.ctrl_root_visible.get() {
            return Ok(BOOL(1));
        }
        let handled = self.should_handle_key(vk);
        debug!("OnTestKeyUp: vk={}, should_handle_key={}", vk.0, handled);
        Ok(BOOL(if handled { 1 } else { 0 }))
    }

    fn OnKeyUp(&self, pic: Ref<'_, ITfContext>, wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        let vk = VIRTUAL_KEY(wparam.0 as u16);
        debug!("OnKeyUp: vk={}", vk.0);

        if vk.0 == VK_CONTROL.0 {
            if self.ctrl_root_visible.get() {
                debug!("  -> Ctrl released, sending HideRoot");
                self.ipc.hide_root();
                self.ctrl_root_visible.set(false);
            }
            return Ok(BOOL(0));
        }

        if vk.0 != VK_SHIFT.0 {
            // Forward other key-ups to rime (ascii_composer / key_binder handle them)
            if !self.should_handle_key(vk) {
                debug!("  -> not handling key up");
                return Ok(BOOL(0));
            }
            let context = pic.as_ref();
            let handled = self.handle_key_up_event(context, vk);
            debug!("  -> key up result: {}", handled);
            return Ok(BOOL(if handled { 1 } else { 0 }));
        }

        if !self.ipc.is_connected() {
            debug!("  -> IPC not connected");
            return Ok(BOOL(0));
        }

        debug!("  -> calling toggle_ascii_mode");
        let response = self.ipc.toggle_ascii_mode();
        debug!("  -> response: {:?}", response);

        if let Some(response) = response {
            if response.success {
                if let Some(ref status) = response.status {
                    debug!(
                        "  -> server ascii_mode: {}, updating local",
                        status.ascii_mode
                    );
                    self.ascii_mode
                        .store(status.ascii_mode, std::sync::atomic::Ordering::Release);
                    let verify = self.ascii_mode.load(std::sync::atomic::Ordering::Acquire);
                    debug!("  -> verified local ascii_mode: {}", verify);
                    self.update_lang_bar();
                }

                let output = RimeOutput::from_response(&response);
                debug!(
                    "  -> output: commit={}, composing={}",
                    output.commit.is_some(),
                    output.composing
                );
                self.set_composing(output.composing);

                if output.commit.is_some() {
                    debug!(
                        "  -> has commit, scheduling edit session to commit and end composition"
                    );
                    if let Some(ctx) = pic.as_ref() {
                        self.schedule_edit_session(ctx, output);
                    }
                } else if !output.composing && self.is_composing() {
                    debug!("  -> composing changed to false, ending composition");
                    self.set_composing(false);
                    if let Some(ctx) = pic.as_ref() {
                        self.schedule_edit_session(
                            ctx,
                            RimeOutput {
                                commit: None,
                                preedit: String::new(),
                                _candidates: Vec::new(),
                                composing: false,
                            },
                        );
                    }
                }

                return Ok(BOOL(1));
            }
        }

        Ok(BOOL(0))
    }

    fn OnPreservedKey(&self, _pic: Ref<'_, ITfContext>, _rguid: *const GUID) -> Result<BOOL> {
        Ok(BOOL(0))
    }
}

#[implement(
    ITfTextInputProcessor,
    ITfActiveLanguageProfileNotifySink,
    ITfThreadFocusSink,
    ITfThreadMgrEventSink,
    ITfKeyEventSink
)]
pub struct XimeTextService {
    thread_mgr: std::cell::RefCell<Option<ITfThreadMgr>>,
    client_id: std::cell::Cell<u32>,
    ipc: IpcClientHandle,
    composing: std::cell::Cell<bool>,
    composition: Arc<std::sync::Mutex<Option<ITfComposition>>>,
    lang_bar_mgr: std::cell::RefCell<Option<ITfLangBarItemMgr>>,
    ascii_mode: crate::language_bar::SharedAsciiMode,
    lang_bar_sink_ref: crate::language_bar::LangBarSinkRef,
    lang_bar_item: std::cell::RefCell<Option<ITfLangBarItemButton>>,
    profile_sink_cookie: std::cell::Cell<u32>,
    profile_source: std::cell::RefCell<Option<ITfSource>>,
    thread_focus_sink_cookie: std::cell::Cell<u32>,
    thread_focus_source: std::cell::RefCell<Option<ITfSource>>,
    thread_mgr_event_sink_cookie: std::cell::Cell<u32>,
    last_input_key: std::cell::Cell<Option<char>>,
    ctrl_root_visible: std::cell::Cell<bool>,
    processing_focus: std::cell::Cell<bool>,
    tray_visible: std::cell::Cell<bool>,
}

pub const GUID_LANG_BAR_ITEM: GUID = GUID_LBI_INPUTMODE;

impl XimeTextService {
    pub fn new() -> Self {
        let ipc = IpcClientHandle::empty();
        debug!(
            "XimeTextService::new() called, IPC state ptr={:p}",
            ipc.state
        );
        Self {
            thread_mgr: std::cell::RefCell::new(None),
            client_id: std::cell::Cell::new(0),
            ipc,
            composing: std::cell::Cell::new(false),
            composition: Arc::new(std::sync::Mutex::new(None)),
            lang_bar_mgr: std::cell::RefCell::new(None),
            ascii_mode: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            lang_bar_sink_ref: Arc::new(std::sync::Mutex::new(None)),
            lang_bar_item: std::cell::RefCell::new(None),
            profile_sink_cookie: std::cell::Cell::new(TF_INVALID_COOKIE),
            profile_source: std::cell::RefCell::new(None),
            thread_focus_sink_cookie: std::cell::Cell::new(TF_INVALID_COOKIE),
            thread_focus_source: std::cell::RefCell::new(None),
            thread_mgr_event_sink_cookie: std::cell::Cell::new(TF_INVALID_COOKIE),
            last_input_key: std::cell::Cell::new(None),
            ctrl_root_visible: std::cell::Cell::new(false),
            processing_focus: std::cell::Cell::new(false),
            tray_visible: std::cell::Cell::new(false),
        }
    }

    fn ensure_ipc(&self) -> std::result::Result<(), winxime_ipc::IpcError> {
        let connected = self.ipc.is_connected();
        debug!(
            "ensure_ipc: is_connected={}, ptr={:p}",
            connected,
            self.ipc.debug_ptr()
        );
        if connected {
            return Ok(());
        }
        debug!("ensure_ipc: attempting connect...");
        match self.ipc.connect() {
            Ok(()) => {
                let (_, response) = self.ipc.start_session();
                if let Some(response) = response {
                    if let Some(status) = response.status {
                        debug!("ensure_ipc: initial ascii_mode={}", status.ascii_mode);
                        self.ascii_mode
                            .store(status.ascii_mode, std::sync::atomic::Ordering::Release);
                    }
                }
                debug!("ensure_ipc: connect OK");
                Ok(())
            }
            Err(e) => {
                debug!("ensure_ipc: connect FAILED: {:?}", e);
                Err(e)
            }
        }
    }
}

impl XimeTextService_Impl {
    fn activate_impl(&self, ptim: Option<&ITfThreadMgr>, tid: u32) -> Result<()> {
        xime_config::init_logging("tsf");
        debug!("Activate called, tid={}", tid);

        *self.thread_mgr.borrow_mut() = ptim.cloned();
        self.client_id.set(tid);

        match self.ensure_ipc() {
            Ok(()) => {
                debug!("IPC connected successfully");
                self.show_tray_icon_guarded();
            }
            Err(e) => debug!("IPC connection failed: {:?}", e),
        }

        if let Some(thread_mgr) = ptim {
            // Register ITfKeyEventSink directly (self implements ITfKeyEventSink)
            if let Ok(kmgr) = thread_mgr.cast::<ITfKeystrokeMgr>() {
                debug!("Got ITfKeystrokeMgr, attempting AdviseKeyEventSink...");
                use windows_core::ComObjectInterface;
                let key_sink_ref = ComObjectInterface::<ITfKeyEventSink>::as_interface_ref(self);
                let key_sink: ITfKeyEventSink = key_sink_ref.to_owned();
                unsafe {
                    if kmgr.AdviseKeyEventSink(tid, &key_sink, true).is_ok() {
                        debug!("AdviseKeyEventSink succeeded");
                    } else {
                        debug!("AdviseKeyEventSink failed");
                    }
                }
            } else {
                debug!("Failed to get ITfKeystrokeMgr");
            }

            if let Ok(lang_bar_mgr) = thread_mgr.cast::<ITfLangBarItemMgr>() {
                debug!("Got ITfLangBarItemMgr, creating LangBarItem...");
                let lang_bar = crate::language_bar::LangBarItemButton::new(
                    GUID_LANG_BAR_ITEM,
                    self.ipc.clone(),
                    self.ascii_mode.clone(),
                    self.lang_bar_sink_ref.clone(),
                );
                let lang_bar_itf: ITfLangBarItemButton = lang_bar.into();
                let add_result = unsafe { lang_bar_mgr.AddItem(&lang_bar_itf) };
                match add_result {
                    Ok(_) => {
                        debug!("LangBarItem added successfully");
                        *self.lang_bar_mgr.borrow_mut() = Some(lang_bar_mgr);
                        *self.lang_bar_item.borrow_mut() = Some(lang_bar_itf);
                    }
                    Err(e) => {
                        debug!("LangBarItem AddItem failed: {:?}", e);
                    }
                }
            } else {
                debug!("Failed to get ITfLangBarItemMgr");
            }

            // Register profile activation sink for input method switching
            if let Ok(source) = thread_mgr.cast::<ITfSource>() {
                debug!("Got ITfSource from thread_mgr, registering sinks...");
                use windows_core::ComObjectInterface;

                // Register ITfActiveLanguageProfileNotifySink
                let profile_sink_ref =
                    ComObjectInterface::<ITfActiveLanguageProfileNotifySink>::as_interface_ref(
                        self,
                    );
                let profile_sink: ITfActiveLanguageProfileNotifySink = profile_sink_ref.to_owned();
                unsafe {
                    if let Ok(cookie) =
                        source.AdviseSink(&ITfActiveLanguageProfileNotifySink::IID, &profile_sink)
                    {
                        debug!("Profile sink registered, cookie={}", cookie);
                        self.profile_sink_cookie.set(cookie);
                        *self.profile_source.borrow_mut() = Some(source.clone());
                    } else {
                        debug!("Failed to AdviseSink for profile");
                    }
                }

                // Register ITfThreadFocusSink
                let thread_focus_sink_ref =
                    ComObjectInterface::<ITfThreadFocusSink>::as_interface_ref(self);
                let thread_focus_sink: ITfThreadFocusSink = thread_focus_sink_ref.to_owned();
                unsafe {
                    if let Ok(cookie) =
                        source.AdviseSink(&ITfThreadFocusSink::IID, &thread_focus_sink)
                    {
                        debug!("Thread focus sink registered, cookie={}", cookie);
                        self.thread_focus_sink_cookie.set(cookie);
                        *self.thread_focus_source.borrow_mut() = Some(source.clone());
                    } else {
                        debug!("Failed to AdviseSink for thread focus");
                    }
                }

                // Register ITfThreadMgrEventSink (for document focus changes)
                let thread_mgr_event_sink_ref =
                    ComObjectInterface::<ITfThreadMgrEventSink>::as_interface_ref(self);
                let thread_mgr_event_sink: ITfThreadMgrEventSink =
                    thread_mgr_event_sink_ref.to_owned();
                unsafe {
                    if let Ok(cookie) =
                        source.AdviseSink(&ITfThreadMgrEventSink::IID, &thread_mgr_event_sink)
                    {
                        debug!("ThreadMgrEventSink registered, cookie={}", cookie);
                        self.thread_mgr_event_sink_cookie.set(cookie);
                    } else {
                        debug!("Failed to AdviseSink for ThreadMgrEventSink");
                    }
                }
            } else {
                debug!("Failed to get ITfSource from thread_mgr");
            }
        } else {
            debug!("No thread_mgr available");
        }

        Ok(())
    }

    fn deactivate_impl(&self) -> Result<()> {
        debug!("Deactivate called");
        self.hide_tray_icon_guarded();

        // Unregister profile sink
        if self.profile_sink_cookie.get() != TF_INVALID_COOKIE {
            if let Some(source) = self.profile_source.borrow_mut().take() {
                unsafe {
                    let _ = source.UnadviseSink(self.profile_sink_cookie.get());
                }
            }
            self.profile_sink_cookie.set(TF_INVALID_COOKIE);
        }

        // Unregister thread focus sink
        if self.thread_focus_sink_cookie.get() != TF_INVALID_COOKIE {
            if let Some(source) = self.thread_focus_source.borrow_mut().take() {
                unsafe {
                    let _ = source.UnadviseSink(self.thread_focus_sink_cookie.get());
                }
            }
            self.thread_focus_sink_cookie.set(TF_INVALID_COOKIE);
        }

        // Unregister thread mgr event sink
        if self.thread_mgr_event_sink_cookie.get() != TF_INVALID_COOKIE {
            if let Some(thread_mgr) = self.thread_mgr.borrow().as_ref() {
                if let Ok(source) = thread_mgr.cast::<ITfSource>() {
                    unsafe {
                        let _ = source.UnadviseSink(self.thread_mgr_event_sink_cookie.get());
                    }
                }
            }
            self.thread_mgr_event_sink_cookie.set(TF_INVALID_COOKIE);
        }

        // UnadviseKeyEventSink using thread_mgr
        if let Some(thread_mgr) = self.thread_mgr.borrow().as_ref() {
            if let Ok(kmgr) = thread_mgr.cast::<ITfKeystrokeMgr>() {
                unsafe {
                    let _ = kmgr.UnadviseKeyEventSink(self.client_id.get());
                }
            }
        }

        if let Some(lang_bar_mgr) = self.lang_bar_mgr.borrow_mut().take() {
            if let Some(lang_bar_item) = self.lang_bar_item.borrow_mut().take() {
                unsafe {
                    if let Err(e) = lang_bar_mgr.RemoveItem(&lang_bar_item) {
                        debug!("RemoveItem failed: {:?}", e);
                    }
                }
            }
        }

        *self.thread_mgr.borrow_mut() = None;
        self.client_id.set(0);
        self.composing.set(false);
        match self.composition.try_lock() {
            Ok(mut guard) => *guard = None,
            Err(std::sync::TryLockError::Poisoned(e)) => *e.into_inner() = None,
            Err(std::sync::TryLockError::WouldBlock) => {}
        }
        Ok(())
    }
}

impl ITfTextInputProcessor_Impl for XimeTextService_Impl {
    fn Activate(&self, ptim: Ref<'_, ITfThreadMgr>, tid: u32) -> Result<()> {
        self.activate_impl(ptim.as_ref(), tid)
    }

    fn Deactivate(&self) -> Result<()> {
        self.deactivate_impl()
    }
}

impl ITfActiveLanguageProfileNotifySink_Impl for XimeTextService_Impl {
    fn OnActivated(
        &self,
        clsid: *const GUID,
        guidprofile: *const GUID,
        factivated: BOOL,
    ) -> Result<()> {
        let clsid_ref = unsafe { clsid.as_ref() };
        let profile_ref = unsafe { guidprofile.as_ref() };

        debug!("ITfActiveLanguageProfileNotifySink::OnActivated: clsid={:?}, profile={:?}, activated={}", clsid_ref, profile_ref, factivated.0);

        let our_clsid = crate::class_factory::CLSID_XIME;
        debug!("  -> our_clsid = {:?}", our_clsid);

        if let Some(clsid_val) = clsid_ref {
            debug!("  -> clsid_val = {:?}", clsid_val);
            debug!("  -> clsid_val == our_clsid? {}", clsid_val == &our_clsid);
            if clsid_val != &our_clsid {
                debug!("  -> Not our TIP, ignoring");
                return Ok(());
            }
            debug!("  -> clsid matches our TIP!");
        } else {
            debug!("  -> clsid_ref is None");
        }

        debug!("  -> factivated.as_bool() = {}", factivated.as_bool());

        if factivated.as_bool() {
            debug!("  -> Our TIP is being activated!");

            if !self.ipc.is_connected() {
                debug!("  -> IPC not connected, attempting reconnect...");
                if self.ipc.connect().is_err() {
                    debug!("  -> Reconnect failed, skipping");
                    return Ok(());
                }
                debug!("  -> Reconnected!");
            }

            if let Some(response) = self.ipc.start_session().1 {
                if let Some(status) = response.status {
                    debug!("  -> ascii_mode from server: {}", status.ascii_mode);
                    self.ascii_mode
                        .store(status.ascii_mode, std::sync::atomic::Ordering::Release);

                    let sink = match self.lang_bar_sink_ref.try_lock() {
                        Ok(g) => g,
                        Err(_) => return Ok(()),
                    };
                    if let Some(ref sink) = *sink {
                        unsafe {
                            let _ = sink.OnUpdate(TF_LBI_STATUS | TF_LBI_ICON | TF_LBI_TEXT);
                        }
                    }
                }
            }

            self.show_tray_icon_guarded();
        } else {
            debug!("  -> Our TIP is being deactivated");
            self.ipc.hide_candidates();
            self.hide_tray_icon_guarded();
        }

        Ok(())
    }
}

impl ITfThreadFocusSink_Impl for XimeTextService_Impl {
    fn OnSetThreadFocus(&self) -> Result<()> {
        // Focus handling is done in ITfThreadMgrEventSink::OnSetFocus
        debug!("ITfThreadFocusSink::OnSetThreadFocus - ignored");
        Ok(())
    }

    fn OnKillThreadFocus(&self) -> Result<()> {
        // Focus handling is done in ITfThreadMgrEventSink::OnSetFocus
        debug!("ITfThreadFocusSink::OnKillThreadFocus - ignored");
        Ok(())
    }
}

impl ITfThreadMgrEventSink_Impl for XimeTextService_Impl {
    fn OnInitDocumentMgr(&self, _pdim: Ref<'_, ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    fn OnUninitDocumentMgr(&self, _pdim: Ref<'_, ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    fn OnSetFocus(
        &self,
        pdimfocus: Ref<'_, ITfDocumentMgr>,
        _pdimprevfocus: Ref<'_, ITfDocumentMgr>,
    ) -> Result<()> {
        debug!(
            "ITfThreadMgrEventSink::OnSetFocus (pdimfocus.is_null={}, processing_focus={})",
            pdimfocus.is_null(),
            self.processing_focus.get()
        );

        // Re-entrancy guard: prevent focus event nesting from STA message pumping
        if self.processing_focus.get() {
            debug!("  -> re-entrant: skipping");
            return Ok(());
        }
        self.processing_focus.set(true);

        if pdimfocus.is_null() {
            debug!("  -> Focus lost (pdimfocus is null)");
            if self.ipc.is_connected() {
                self.ipc.focus_out();
            }
            self.abort_composition();
            self.hide_tray_icon_guarded();
            self.processing_focus.set(false);
            return Ok(());
        }

        debug!("  -> Focus gained (pdimfocus is valid)");

        if !self.ipc.is_connected() {
            debug!("  -> IPC not connected, attempting reconnect...");
            if self.ipc.connect().is_err() {
                debug!("  -> Reconnect failed");
                self.processing_focus.set(false);
                return Ok(());
            }
            debug!("  -> Reconnected!");
            // start_session is already called inside connect() -> ensure_ipc
        }

        // Sync ascii_mode from server status
        if let Some(response) = self.ipc.start_session().1 {
            if let Some(status) = response.status {
                debug!("  -> ascii_mode from server: {}", status.ascii_mode);
                self.ascii_mode
                    .store(status.ascii_mode, std::sync::atomic::Ordering::Release);

                let sink = match self.lang_bar_sink_ref.try_lock() {
                    Ok(g) => g,
                    Err(_) => {
                        self.processing_focus.set(false);
                        return Ok(());
                    }
                };
                if let Some(ref sink) = *sink {
                    unsafe {
                        let _ = sink.OnUpdate(TF_LBI_STATUS | TF_LBI_ICON | TF_LBI_TEXT);
                    }
                }
            }
        }
        self.ipc.focus_in();
        self.show_tray_icon_guarded();

        self.processing_focus.set(false);
        Ok(())
    }

    fn OnPushContext(&self, _pic: Ref<'_, ITfContext>) -> Result<()> {
        Ok(())
    }

    fn OnPopContext(&self, _pic: Ref<'_, ITfContext>) -> Result<()> {
        Ok(())
    }
}
