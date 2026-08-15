use crate::class_factory::CLSID_XIME;
use std::sync::atomic::{AtomicUsize, Ordering};
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::{GetModuleFileNameW, GetProcAddress, LoadLibraryW};
use windows_core::*;

static DLL_MODULE: AtomicUsize = AtomicUsize::new(0);

fn get_dll_module() -> HINSTANCE {
    HINSTANCE(DLL_MODULE.load(Ordering::Acquire) as *mut _)
}

fn clsid_str() -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        CLSID_XIME.data1,
        CLSID_XIME.data2,
        CLSID_XIME.data3,
        CLSID_XIME.data4[0],
        CLSID_XIME.data4[1],
        CLSID_XIME.data4[2],
        CLSID_XIME.data4[3],
        CLSID_XIME.data4[4],
        CLSID_XIME.data4[5],
        CLSID_XIME.data4[6],
        CLSID_XIME.data4[7],
    )
}

fn get_module_path() -> String {
    unsafe {
        let mut buf = [0u16; 260];
        let len = GetModuleFileNameW(Some(HMODULE(get_dll_module().0)), &mut buf);
        if len > 0 {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            String::new()
        }
    }
}

#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    if rclsid.is_null() || riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    if *rclsid != CLSID_XIME {
        return HRESULT(-2147221231);
    }
    let factory = crate::ClassFactory;
    let unknown: IUnknown = factory.into();
    Interface::query(&unknown, riid, ppv)
}

static ACTIVE_INSTANCES: AtomicUsize = AtomicUsize::new(0);

pub fn increment_instance_count() {
    ACTIVE_INSTANCES.fetch_add(1, Ordering::AcqRel);
}

pub fn decrement_instance_count() {
    ACTIVE_INSTANCES.fetch_sub(1, Ordering::AcqRel);
}

#[no_mangle]
pub unsafe extern "system" fn DllCanUnloadNow() -> HRESULT {
    if ACTIVE_INSTANCES.load(Ordering::Acquire) > 0 {
        S_FALSE
    } else {
        S_OK
    }
}

#[no_mangle]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    match do_register() {
        Ok(_) => S_OK,
        Err(e) => {
            eprintln!("DllRegisterServer failed: {}", e);
            HRESULT(-1i32)
        }
    }
}

#[no_mangle]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    do_unregister();
    S_OK
}

#[no_mangle]
pub unsafe extern "system" fn DllMain(
    hinst: HINSTANCE,
    reason: u32,
    _reserved: *mut core::ffi::c_void,
) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        DLL_MODULE.store(hinst.0 as usize, Ordering::Release);
        crate::language_bar::set_instance(hinst);
        // Use simple file logging for DLL
        let log_path = std::env::var("TEMP")
            .map(|p| std::path::PathBuf::from(p).join("winxime").join("tsf.log"))
            .unwrap_or_else(|_| std::path::PathBuf::from("tsf.log"));
        std::fs::create_dir_all(log_path.parent().unwrap()).ok();
        // Simple initialization without tracing_appender
        xime_config::init_logging("tsf");
    }
    BOOL(1)
}

const DLL_PROCESS_ATTACH: u32 = 1;

fn uninstall_layout() {
    let install_str = format!("0804:{}{{{}}}", clsid_str(), clsid_str(),);
    let wide: Vec<u16> = install_str
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        if let Ok(module) = LoadLibraryW(w!("input.dll")) {
            if let Some(func) = std::mem::transmute::<
                _,
                Option<unsafe extern "system" fn(*const u16, u32) -> BOOL>,
            >(GetProcAddress(module, s!("InstallLayoutOrTip")))
            {
                let _ = func(wide.as_ptr(), 1);
            }
        }
    }
}

fn do_register() -> Result<()> {
    use winreg::enums::*;
    use winreg::RegKey;

    let cs = clsid_str();
    let module_path = get_module_path();
    let name = "曦码·曜输入法";

    eprintln!("[Xime] DllRegisterServer: CLSID={}", cs);
    eprintln!("[Xime] DllRegisterServer: module_path={}", module_path);

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let software = hklm
        .create_subkey("Software")
        .map_err(|e| {
            eprintln!("[Xime] ERROR: Failed to open HKLM\\Software: {}", e);
            e
        })?
        .0;

    let classes = software
        .create_subkey("Classes")
        .map_err(|e| {
            eprintln!(
                "[Xime] ERROR: Failed to open HKLM\\Software\\Classes: {}",
                e
            );
            e
        })?
        .0;

    eprintln!("[Xime] Step 1: Creating CLSID registry key...");
    let (clsid_key, _) = classes
        .create_subkey(&format!("CLSID\\{}", cs))
        .map_err(|e| {
            eprintln!("[Xime] ERROR: Failed to create CLSID key: {}", e);
            e
        })?;

    eprintln!("[Xime] Step 2: Setting default value...");
    clsid_key.set_value("", &name).map_err(|e| {
        eprintln!("[Xime] ERROR: Failed to set default value: {}", e);
        e
    })?;

    eprintln!("[Xime] Step 3: Creating InprocServer32...");
    let (inproc, _) = clsid_key.create_subkey("InprocServer32").map_err(|e| {
        eprintln!("[Xime] ERROR: Failed to create InprocServer32: {}", e);
        e
    })?;
    inproc.set_value("", &module_path).map_err(|e| {
        eprintln!("[Xime] ERROR: Failed to set module path: {}", e);
        e
    })?;
    inproc
        .set_value("ThreadingModel", &"Apartment")
        .map_err(|e| {
            eprintln!("[Xime] ERROR: Failed to set ThreadingModel: {}", e);
            e
        })?;

    let _ = classes.create_subkey(&format!(
        "CLSID\\{}\\Implemented Categories\\{{34745C63-B2F0-4784-8B67-5E12C8701A31}}",
        cs
    ));

    eprintln!("[Xime] DllRegisterServer completed successfully!");
    eprintln!("[Xime] Note: Run winxime-tsf-register to register TSF profile and icon");
    Ok(())
}

fn do_unregister() {
    use winreg::enums::*;
    use winreg::RegKey;

    let cs = clsid_str();

    uninstall_layout();

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let _ = hklm.delete_subkey_all(&format!("Software\\Classes\\CLSID\\{}", cs));
    let _ = hklm.delete_subkey_all(&format!("Software\\Microsoft\\CTF\\TIP\\{}", cs));
}
