use tracing::{info, warn};
use windows::{
    core::*,
    Win32::{
        Foundation::E_FAIL,
        Globalization::*,
        System::{
            Com::*,
            LibraryLoader::{GetProcAddress, LoadLibraryW},
        },
        UI::{
            Input::KeyboardAndMouse::HKL,
            TextServices::*,
        },
    },
};

windows::core::link!("input.dll" "system" fn InstallLayoutOrTip(psz: *const u16, dwFlags: u32));

const ILOT_INSTALL: u32 = 0x00000000;

const CLSID_XIME: GUID = GUID::from_u128(0x5C1E4D8A_F3B2_4A7E_9CD1_2A3B4C5D6E7F);
const PROFILE_GUID: GUID = GUID::from_u128(0x5C1E4D8A_F3B2_4A7E_9CD1_2A3B4C5D6E7F);

const CATEGORIES: [GUID; 7] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    GUID_TFCAT_TIPCAP_COMLESS,
];

fn register_dll() -> Result<()> {
    let exe_path = std::env::current_exe().map_err(|_| Error::from(E_FAIL))?;
    let exe_dir = exe_path.parent().ok_or(Error::from(E_FAIL))?;
    let dll_path = exe_dir.join("winxime_tsf.dll");
    let dll_str = dll_path.to_string_lossy();

    unsafe {
        let wide: Vec<u16> = dll_str.encode_utf16().chain(std::iter::once(0)).collect();
        let hmodule = LoadLibraryW(PCWSTR::from_raw(wide.as_ptr()))?;
        let proc = GetProcAddress(hmodule, s!("DllRegisterServer"))
            .ok_or(Error::from(E_FAIL))?;
        let func: extern "system" fn() -> HRESULT = std::mem::transmute(proc);
        let hr = func();
        if hr.is_err() {
            return Err(Error::from(hr));
        }
    }
    Ok(())
}

fn find_icon_path() -> String {
    let exe_path = std::env::current_exe().ok();
    if let Some(path) = exe_path {
        if let Some(dir) = path.parent() {
            let ico = dir.join("icon.ico");
            if ico.exists() {
                return ico.to_string_lossy().to_string();
            }
            let resources_ico = dir.join("resources").join("icon.ico");
            if resources_ico.exists() {
                return resources_ico.to_string_lossy().to_string();
            }
        }
    }
    "icon.ico".to_string()
}

fn register_profile() -> Result<()> {
    register_dll()?;
    info!("COM server registered");

    unsafe {
        let mgr: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;

        let mut lcid = LocaleNameToLCID(w!("zh-CN"), 0);
        if matches!(lcid, 0 | 0x0C00 | 0x1000) {
            lcid = 0x804;
        }

        let icon_path = find_icon_path();
        let icon_wide: Vec<u16> = icon_path.encode_utf16().chain(std::iter::once(0)).collect();

        mgr.RegisterProfile(
            &CLSID_XIME,
            lcid as u16,
            &PROFILE_GUID,
            w!("曦码·曜输入法").as_wide(),
            &icon_wide,
            0,
            HKL::default(),
            0,
            false,
            0,
        )?;

        info!("TSF profile registered");

        let cat_mgr: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        for tfcat in &CATEGORIES {
            cat_mgr.RegisterCategory(&CLSID_XIME, tfcat, &CLSID_XIME)?;
        }

        info!("TSF categories registered");

        let tip_desc = w!("0x0804:{5C1E4D8A-F3B2-4A7E-9CD1-2A3B4C5D6E7F}{5C1E4D8A-F3B2-4A7E-9CD1-2A3B4C5D6E7F}");
        InstallLayoutOrTip(tip_desc.as_ptr(), ILOT_INSTALL);
        info!("Input method enabled");
    }
    Ok(())
}

pub fn ensure_registered() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    match register_profile() {
        Ok(_) => info!("TSF registration complete"),
        Err(e) => warn!("TSF registration skipped (may already be registered): {e}"),
    }
}
