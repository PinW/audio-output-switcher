use std::ffi::c_void;

use windows::core::{Interface, GUID, HRESULT, PCWSTR, PWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{
    eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ,
};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;

// Undocumented IPolicyConfig COM interface GUIDs
const CLSID_POLICY_CONFIG_CLIENT: GUID =
    GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
const IID_IPOLICY_CONFIG: GUID =
    GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);

pub struct AudioDevice {
    pub id: String,
    pub name: String,
}

/// List all active audio output (render) devices.
pub fn list_devices() -> windows::core::Result<Vec<AudioDevice>> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let collection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
        let count = collection.GetCount()?;

        let mut devices = Vec::new();
        for i in 0..count {
            let device = collection.Item(i)?;

            // Get device ID
            let id_pwstr: PWSTR = device.GetId()?;
            let id = id_pwstr.to_string()?;
            CoTaskMemFree(Some(id_pwstr.0 as *const c_void));

            // Get friendly name from property store
            let store: IPropertyStore = device.OpenPropertyStore(STGM_READ)?;
            let prop = store.GetValue(&PKEY_Device_FriendlyName)?;
            let name = prop.to_string();

            devices.push(AudioDevice { id, name });
        }

        Ok(devices)
    }
}

/// Get the endpoint ID of the current default audio output device.
pub fn get_default_device_id() -> windows::core::Result<String> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let id_pwstr: PWSTR = device.GetId()?;
        let id = id_pwstr.to_string()?;
        CoTaskMemFree(Some(id_pwstr.0 as *const c_void));
        Ok(id)
    }
}

/// Set the default audio output device for all roles (console, multimedia, communications).
///
/// Uses the undocumented IPolicyConfig COM interface via raw vtable access.
pub fn set_default_device(device_id: &str) -> windows::core::Result<()> {
    unsafe {
        // Encode device_id as null-terminated UTF-16
        let wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
        let pcwstr = PCWSTR(wide.as_ptr());

        // Create CPolicyConfigClient and QueryInterface for IPolicyConfig
        let unknown: windows::core::IUnknown =
            CoCreateInstance(&CLSID_POLICY_CONFIG_CLIENT, None, CLSCTX_ALL)?;
        let raw = unknown.as_raw();

        let mut policy_config: *mut c_void = std::ptr::null_mut();
        let vtable = *(raw as *const *const *const c_void);
        let query_interface: unsafe extern "system" fn(
            *mut c_void,
            *const GUID,
            *mut *mut c_void,
        ) -> HRESULT = std::mem::transmute(*vtable.add(0));
        query_interface(raw, &IID_IPOLICY_CONFIG, &mut policy_config).ok()?;

        // Access IPolicyConfig vtable
        let pc_vtable = *(policy_config as *const *const *const c_void);

        // SetDefaultEndpoint is at vtable index 13:
        //   IUnknown (3 methods) + 10 IPolicyConfig methods before SetDefaultEndpoint
        type SetDefaultEndpointFn =
            unsafe extern "system" fn(*mut c_void, PCWSTR, u32) -> HRESULT;
        let set_default_endpoint: SetDefaultEndpointFn =
            std::mem::transmute(*pc_vtable.add(13));

        // Set for all 3 roles: eConsole=0, eMultimedia=1, eCommunications=2
        for role in 0..3u32 {
            set_default_endpoint(policy_config, pcwstr, role).ok()?;
        }

        // Release IPolicyConfig
        type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;
        let release: ReleaseFn = std::mem::transmute(*pc_vtable.add(2));
        release(policy_config);

        Ok(())
    }
}
