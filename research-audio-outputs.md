# Research: Programmatic Control of Audio Output Devices on Windows

**Date:** 2026-02-27

---

## Table of Contents

1. [Overview](#overview)
2. [Enumerating Audio Output Devices](#1-enumerating-audio-output-devices)
3. [Switching the Default Audio Output](#2-switching-the-default-audio-output)
4. [Available APIs and Libraries](#3-available-apis-and-libraries)
   - [Windows Core Audio API (MMDevice, PolicyConfig)](#31-windows-core-audio-api-mmdevice-policyconfig)
   - [PowerShell (AudioDeviceCmdlets)](#32-powershell-audiodevicecmdlets)
   - [Python Libraries](#33-python-libraries)
   - [C# / .NET Approaches](#34-c--net-approaches)
   - [Standalone CLI Tools](#35-standalone-cli-tools)
5. [Dynamic Detection of Device Changes](#4-dynamic-detection-of-device-changes)
6. [Gotchas and Limitations](#5-gotchas-and-limitations)
7. [Recommendation Summary](#6-recommendation-summary)

---

## Overview

Windows does **not** provide an official, documented public API for changing the default audio output device. All approaches that set the default device rely on one of:

- The **undocumented `IPolicyConfig` COM interface** (reverse-engineered, used by virtually every tool that does this)
- **Wrapper libraries/modules** that internally call `IPolicyConfig` (AudioDeviceCmdlets, SoundSwitch, NirCmd, etc.)

Enumerating devices, on the other hand, is fully supported through the documented **MMDevice API** (`IMMDeviceEnumerator`).

---

## 1. Enumerating Audio Output Devices

### Core Concept

Windows audio devices are modeled as **audio endpoints**. Each endpoint has:

- **Endpoint ID string** -- a unique opaque identifier (e.g., `{0.0.0.00000000}.{guid}`)
- **Friendly name** -- human-readable (e.g., "Speakers (Realtek High Definition Audio)")
- **Data flow** -- `eRender` (output/playback) or `eCapture` (input/recording)
- **Device state** -- `ACTIVE`, `DISABLED`, `NOTPRESENT`, or `UNPLUGGED`
- **Device roles** -- `eConsole` (system sounds), `eMultimedia` (media playback), `eCommunications` (voice chat)

### MMDevice API (C/C++)

The documented way to enumerate devices:

```cpp
#include <mmdeviceapi.h>
#include <functiondiscoverykeys_devpkey.h>

CoInitialize(NULL);

IMMDeviceEnumerator *pEnumerator = NULL;
CoCreateInstance(__uuidof(MMDeviceEnumerator), NULL, CLSCTX_ALL,
                 __uuidof(IMMDeviceEnumerator), (void**)&pEnumerator);

// Enumerate all active render (output) devices
IMMDeviceCollection *pCollection = NULL;
pEnumerator->EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE, &pCollection);

UINT count;
pCollection->GetCount(&count);

for (UINT i = 0; i < count; i++) {
    IMMDevice *pDevice = NULL;
    pCollection->Item(i, &pDevice);

    // Get endpoint ID
    LPWSTR pwszID = NULL;
    pDevice->GetId(&pwszID);

    // Get friendly name
    IPropertyStore *pProps = NULL;
    pDevice->OpenPropertyStore(STGM_READ, &pProps);
    PROPVARIANT varName;
    PropVariantInit(&varName);
    pProps->GetValue(PKEY_Device_FriendlyName, &varName);

    printf("Device %d: %S (ID: %S)\n", i, varName.pwszVal, pwszID);

    PropVariantClear(&varName);
    CoTaskMemFree(pwszID);
    pProps->Release();
    pDevice->Release();
}

// Get current default device
IMMDevice *pDefault = NULL;
pEnumerator->GetDefaultAudioEndpoint(eRender, eConsole, &pDefault);
LPWSTR defaultId = NULL;
pDefault->GetId(&defaultId);
printf("Default device ID: %S\n", defaultId);
```

### PowerShell (AudioDeviceCmdlets)

```powershell
# List all enabled audio devices
Get-AudioDevice -List

# Filter to playback devices only
Get-AudioDevice -List | Where-Object { $_.Type -eq "Playback" }

# Get the current default playback device
Get-AudioDevice -Playback

# Get the current default communication device
Get-AudioDevice -PlaybackCommunication
```

### Python (pycaw)

```python
from pycaw.pycaw import AudioUtilities

# Get all active audio devices
devices = AudioUtilities.GetAllDevices()
for device in devices:
    print(f"Name: {device.FriendlyName}, ID: {device.id}, State: {device.state}")

# Get the default speaker device
speakers = AudioUtilities.GetSpeakers()
```

### Python (sounddevice)

```python
import sounddevice as sd

# List all audio devices
print(sd.query_devices())

# Get default input/output devices
print(sd.default.device)
```

### Python (ctypes + DirectSound)

A lower-level approach using DirectSound enumeration:

```python
import ctypes
import ctypes.wintypes as wt

dsound = ctypes.windll.LoadLibrary("dsound.dll")
ole32 = ctypes.oledll.ole32

LPDSENUMCALLBACK = ctypes.WINFUNCTYPE(
    wt.BOOL, ctypes.c_void_p, wt.LPCWSTR, wt.LPCWSTR, ctypes.c_void_p
)

def get_playback_devices():
    devices = []
    def callback(lpGUID, lpszDesc, lpszDrvName, _unused):
        if lpGUID is not None:
            buf = ctypes.create_unicode_buffer(500)
            ole32.StringFromGUID2(ctypes.c_int64(lpGUID), ctypes.byref(buf), 500)
            devices.append((buf.value, lpszDesc, lpszDrvName))
        return True
    dsound.DirectSoundEnumerateW(LPDSENUMCALLBACK(callback), None)
    return devices

for guid, desc, drv in get_playback_devices():
    print(f"{guid}: {desc} | {drv}")
```

---

## 2. Switching the Default Audio Output

### The Core Problem

Microsoft has never documented an API to set the default audio device. The system stores default device preferences in the registry at:

```
HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\{GUID}
```

However, writing to the registry directly does not work reliably -- the change is not picked up by the audio subsystem without additional signaling.

### The Universal Solution: IPolicyConfig (Undocumented COM Interface)

Virtually every tool that switches the default audio device uses the **undocumented `IPolicyConfig` COM interface**. This has been reverse-engineered from Windows binaries and has worked from Windows Vista through Windows 11 (with occasional GUID changes between major OS versions).

**Key method:** `SetDefaultEndpoint(LPCWSTR deviceId, ERole role)`

**Device roles to set:**
| Role | Value | Description |
|------|-------|-------------|
| `eConsole` | 0 | Default device for system sounds and most applications |
| `eMultimedia` | 1 | Default device for media playback |
| `eCommunications` | 2 | Default device for voice communications (e.g., Teams, Discord) |

To fully switch a device, you typically call `SetDefaultEndpoint` three times -- once for each role.

---

## 3. Available APIs and Libraries

### 3.1 Windows Core Audio API (MMDevice, PolicyConfig)

#### Documented APIs (Enumeration and Monitoring)

| Interface | Purpose | Header |
|-----------|---------|--------|
| `IMMDeviceEnumerator` | Enumerate devices, get default device | `mmdeviceapi.h` |
| `IMMDeviceCollection` | Collection of endpoint devices | `mmdeviceapi.h` |
| `IMMDevice` | Single endpoint device | `mmdeviceapi.h` |
| `IMMNotificationClient` | Device change notifications | `mmdeviceapi.h` |
| `IAudioEndpointVolume` | Volume control on an endpoint | `endpointvolume.h` |

#### Undocumented APIs (Setting Default Device)

**IPolicyConfig** (Windows 7+):
- Interface GUID: `{f8679f50-850a-41cf-9c72-430f290290c8}`
- Class GUID (`CPolicyConfigClient`): `{870af99c-171d-4f9e-af0d-e63df40c2bc9}`

**IPolicyConfigVista** (Windows Vista):
- Interface GUID: `{568b9108-44bf-40b4-9006-86afe5b5a620}`
- Class GUID (`CPolicyConfigVistaClient`): `{294935CE-F637-4E7C-A41B-AB255460B862}`

**C++ usage (PolicyConfig.h):**

```cpp
#include "PolicyConfig.h"       // Undocumented header (community-maintained)
#include <mmdeviceapi.h>

HRESULT SetDefaultAudioPlaybackDevice(LPCWSTR deviceId) {
    IPolicyConfig *pPolicyConfig;
    HRESULT hr = CoCreateInstance(
        __uuidof(CPolicyConfigClient), NULL, CLSCTX_ALL,
        __uuidof(IPolicyConfig), (LPVOID *)&pPolicyConfig);

    if (SUCCEEDED(hr)) {
        hr = pPolicyConfig->SetDefaultEndpoint(deviceId, eConsole);
        hr = pPolicyConfig->SetDefaultEndpoint(deviceId, eMultimedia);
        hr = pPolicyConfig->SetDefaultEndpoint(deviceId, eCommunications);
        pPolicyConfig->Release();
    }
    return hr;
}
```

The `PolicyConfig.h` header is not shipped with the Windows SDK. It must be obtained from community sources such as:
- https://gist.github.com/VonLatvala/021b048297973a3d47ed7e39dfe2adf0
- https://github.com/Belphemur/AudioEndPointLibrary/blob/master/DefSound/PolicyConfig.h

---

### 3.2 PowerShell (AudioDeviceCmdlets)

**Repository:** https://github.com/frgnca/AudioDeviceCmdlets
**PowerShell Gallery:** https://www.powershellgallery.com/packages/AudioDeviceCmdlets/3.1.0.2

The simplest approach for scripting. No dependencies. Uses IPolicyConfig internally.

#### Installation

```powershell
# Run PowerShell as Administrator
Install-Module -Name AudioDeviceCmdlets
```

#### Available Cmdlets

| Cmdlet | Key Parameters | Description |
|--------|---------------|-------------|
| `Get-AudioDevice` | `-List`, `-Playback`, `-Recording`, `-ID`, `-Index` | List/get devices |
| `Set-AudioDevice` | `<AudioDevice>`, `-ID`, `-Index`, `-DefaultOnly`, `-CommunicationOnly` | Set default device |
| `Write-AudioDevice` | `-PlaybackMeter`, `-RecordingMeter` | Monitor audio levels |

#### Usage Examples

```powershell
# List all audio devices
Get-AudioDevice -List

# Get current default playback device
Get-AudioDevice -Playback

# Set default device by piping
Get-AudioDevice -List | Where-Object { $_.Name -like "*Headphones*" } | Set-AudioDevice

# Set default device by index
Set-AudioDevice -Index 3

# Set ONLY as default device (not communication device)
Set-AudioDevice -ID "..." -DefaultOnly

# Set ONLY as communication device
Set-AudioDevice -ID "..." -CommunicationOnly

# Toggle between two devices
$current = Get-AudioDevice -Playback
if ($current.Name -like "*Speakers*") {
    Get-AudioDevice -List | Where-Object { $_.Name -like "*Headphones*" -and $_.Type -eq "Playback" } | Set-AudioDevice
} else {
    Get-AudioDevice -List | Where-Object { $_.Name -like "*Speakers*" -and $_.Type -eq "Playback" } | Set-AudioDevice
}

# Volume control
Set-AudioDevice -PlaybackVolume 50
Get-AudioDevice -PlaybackVolume
Set-AudioDevice -PlaybackMuteToggle
```

#### Calling from WSL

```bash
# List devices from WSL
powershell.exe -Command "Get-AudioDevice -List"

# Switch device from WSL
powershell.exe -Command "Get-AudioDevice -List | Where-Object { \$_.Name -like '*Headphones*' -and \$_.Type -eq 'Playback' } | Set-AudioDevice"
```

---

### 3.3 Python Libraries

#### pycaw (Python Core Audio Windows)

**Repository:** https://github.com/AndreMiras/pycaw
**Install:** `pip install pycaw`
**Stars:** 436 | **License:** MIT

**Capabilities:**
- Enumerate audio devices and sessions
- Control volume (per-device and per-session)
- Mute/unmute
- Get device properties

**Limitation:** pycaw does NOT natively support switching the default audio device. It wraps the documented MMDevice API, not IPolicyConfig.

```python
from pycaw.pycaw import AudioUtilities, IAudioEndpointVolume
from comtypes import CLSCTX_ALL

# Get the default speaker and control volume
speakers = AudioUtilities.GetSpeakers()
interface = speakers.Activate(IAudioEndpointVolume._iid_, CLSCTX_ALL, None)
volume = interface.QueryInterface(IAudioEndpointVolume)

# Get/set volume
current_volume = volume.GetMasterVolumeLevelScalar()  # 0.0 to 1.0
volume.SetMasterVolumeLevelScalar(0.5, None)           # Set to 50%
volume.SetMute(1, None)                                 # Mute

# List all devices
devices = AudioUtilities.GetAllDevices()
for d in devices:
    print(f"{d.FriendlyName} -- {d.id}")
```

#### pyaudiodevice

**Install:** `pip install pyaudiodevice`
**Requires:** Python 3.6+

This newer library wraps IPolicyConfig and **does support switching the default audio device**.

```python
from pyaudiodevice import AudioCommon, DefaultPlayback

# List all devices
ac = AudioCommon()
devices = ac.get_audio_device_list()
for dev in devices:
    print(dev)

# Get current default
default = ac.get_default_device()
print(f"Current default: {default}")

# Set a new default device
ac.set_default_audio_device(target_device)

# Set as communication device
ac.set_default_communication_audio_device(target_device)

# Volume control via DefaultPlayback
dp = DefaultPlayback()
dp.set_volume(0.75)       # 0.0 to 1.0
dp.toggle_mute()
print(dp.get_volume())
print(dp.get_is_mute())
```

#### Python + comtypes (Direct IPolicyConfig Access)

For full control, you can call IPolicyConfig directly through comtypes:

```python
import comtypes
from comtypes import GUID, HRESULT, COMMETHOD
from ctypes import POINTER
from ctypes.wintypes import LPCWSTR, DWORD
from enum import IntEnum

class ERole(IntEnum):
    eConsole = 0
    eMultimedia = 1
    eCommunications = 2

class EDataFlow(IntEnum):
    eRender = 0
    eCapture = 1
    eAll = 2

# IPolicyConfig interface definition
class IPolicyConfig(comtypes.IUnknown):
    _iid_ = GUID('{f8679f50-850a-41cf-9c72-430f290290c8}')
    _methods_ = [
        COMMETHOD([], HRESULT, 'GetMixFormat',
                  (['in'], LPCWSTR, 'pwstrDeviceId'),
                  (['out'], POINTER(comtypes.c_void_p), 'ppFormat')),
        COMMETHOD([], HRESULT, 'GetDeviceFormat',
                  (['in'], LPCWSTR, 'pwstrDeviceId'),
                  (['in'], comtypes.c_int, 'bDefault'),
                  (['out'], POINTER(comtypes.c_void_p), 'ppFormat')),
        COMMETHOD([], HRESULT, 'ResetDeviceFormat',
                  (['in'], LPCWSTR, 'pwstrDeviceId')),
        COMMETHOD([], HRESULT, 'SetDeviceFormat',
                  (['in'], LPCWSTR, 'pwstrDeviceId'),
                  (['in'], comtypes.c_void_p, 'pEndpointFormat'),
                  (['in'], comtypes.c_void_p, 'pMixFormat')),
        COMMETHOD([], HRESULT, 'GetProcessingPeriod',
                  (['in'], LPCWSTR, 'pwstrDeviceId'),
                  (['in'], comtypes.c_int, 'bDefault'),
                  (['out'], POINTER(comtypes.c_longlong), 'pmftDefaultPeriod'),
                  (['out'], POINTER(comtypes.c_longlong), 'pmftMinimumPeriod')),
        COMMETHOD([], HRESULT, 'SetProcessingPeriod',
                  (['in'], LPCWSTR, 'pwstrDeviceId'),
                  (['in'], comtypes.c_longlong, 'pmftPeriod')),
        COMMETHOD([], HRESULT, 'GetShareMode',
                  (['in'], LPCWSTR, 'pwstrDeviceId'),
                  (['out'], POINTER(comtypes.c_void_p), 'pMode')),
        COMMETHOD([], HRESULT, 'SetShareMode',
                  (['in'], LPCWSTR, 'pwstrDeviceId'),
                  (['in'], comtypes.c_void_p, 'pMode')),
        COMMETHOD([], HRESULT, 'GetPropertyValue',
                  (['in'], LPCWSTR, 'pwstrDeviceId'),
                  (['in'], comtypes.c_void_p, 'key'),
                  (['out'], POINTER(comtypes.c_void_p), 'pv')),
        COMMETHOD([], HRESULT, 'SetPropertyValue',
                  (['in'], LPCWSTR, 'pwstrDeviceId'),
                  (['in'], comtypes.c_void_p, 'key'),
                  (['in'], comtypes.c_void_p, 'pv')),
        COMMETHOD([], HRESULT, 'SetDefaultEndpoint',
                  (['in'], LPCWSTR, 'pwstrDeviceId'),
                  (['in'], DWORD, 'eRole')),
        COMMETHOD([], HRESULT, 'SetEndpointVisibility',
                  (['in'], LPCWSTR, 'pwstrDeviceId'),
                  (['in'], comtypes.c_int, 'bVisible')),
    ]

CLSID_CPolicyConfigClient = GUID('{870af99c-171d-4f9e-af0d-e63df40c2bc9}')

def set_default_audio_device(device_id: str):
    """Set the default audio output device by its endpoint ID string."""
    comtypes.CoInitialize()
    try:
        policy_config = comtypes.CoCreateInstance(
            CLSID_CPolicyConfigClient,
            IPolicyConfig,
            comtypes.CLSCTX_ALL
        )
        for role in ERole:
            policy_config.SetDefaultEndpoint(device_id, role.value)
    finally:
        comtypes.CoUninitialize()

def list_and_switch():
    """Example: list devices and switch to a specific one."""
    from pycaw.pycaw import AudioUtilities

    devices = AudioUtilities.GetAllDevices()
    for i, dev in enumerate(devices):
        print(f"[{i}] {dev.FriendlyName}  -->  {dev.id}")

    # Switch to device at index 1 (example)
    if len(devices) > 1:
        set_default_audio_device(devices[1].id)
        print(f"Switched to: {devices[1].FriendlyName}")
```

#### pyWinCoreAudio

**Repository:** https://github.com/kdschlosser/pyWinCoreAudio

A more comprehensive Python wrapper for the entire Windows Core Audio API. Supports device enumeration, default device switching, volume control, and event callbacks. Less widely adopted than pycaw but more feature-complete for device management.

---

### 3.4 C# / .NET Approaches

#### NAudio (Enumeration Only)

NAudio wraps the documented MMDevice API. It can enumerate devices and control volume but does **not** include `SetDefaultEndpoint`.

```csharp
using NAudio.CoreAudioApi;

var enumerator = new MMDeviceEnumerator();

// List all active render devices
foreach (var device in enumerator.EnumerateAudioEndPoints(DataFlow.Render, DeviceState.Active))
{
    Console.WriteLine($"{device.FriendlyName} (ID: {device.ID})");
}

// Get default device
var defaultDevice = enumerator.GetDefaultAudioEndpoint(DataFlow.Render, Role.Console);
Console.WriteLine($"Default: {defaultDevice.FriendlyName}");
```

#### Direct IPolicyConfig via COM Interop (Full Switch Support)

```csharp
using System;
using System.Runtime.InteropServices;

// Device role enum
public enum ERole : uint
{
    eConsole = 0,
    eMultimedia = 1,
    eCommunications = 2
}

// Undocumented IPolicyConfig COM interface
[ComImport]
[Guid("f8679f50-850a-41cf-9c72-430f290290c8")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IPolicyConfig
{
    [PreserveSig] int GetMixFormat(string pwstrDeviceId, IntPtr ppFormat);
    [PreserveSig] int GetDeviceFormat(string pwstrDeviceId, bool bDefault, IntPtr ppFormat);
    [PreserveSig] int ResetDeviceFormat(string pwstrDeviceId);
    [PreserveSig] int SetDeviceFormat(string pwstrDeviceId, IntPtr pEndpointFormat, IntPtr MixFormat);
    [PreserveSig] int GetProcessingPeriod(string pwstrDeviceId, bool bDefault, IntPtr pmftDefaultPeriod, IntPtr pmftMinimumPeriod);
    [PreserveSig] int SetProcessingPeriod(string pwstrDeviceId, IntPtr pmftPeriod);
    [PreserveSig] int GetShareMode(string pwstrDeviceId, IntPtr pMode);
    [PreserveSig] int SetShareMode(string pwstrDeviceId, IntPtr pMode);
    [PreserveSig] int GetPropertyValue(string pwstrDeviceId, IntPtr key, IntPtr pv);
    [PreserveSig] int SetPropertyValue(string pwstrDeviceId, IntPtr key, IntPtr pv);
    [PreserveSig] int SetDefaultEndpoint(string pwstrDeviceId, ERole eRole);
    [PreserveSig] int SetEndpointVisibility(string pwstrDeviceId, bool bVisible);
}

[ComImport]
[Guid("870af99c-171d-4f9e-af0d-e63df40c2bc9")]
internal class CPolicyConfigClient { }

public class AudioDeviceSwitcher
{
    public static void SetDefaultDevice(string deviceId)
    {
        var policyConfig = new CPolicyConfigClient() as IPolicyConfig;
        if (policyConfig != null)
        {
            policyConfig.SetDefaultEndpoint(deviceId, ERole.eConsole);
            policyConfig.SetDefaultEndpoint(deviceId, ERole.eMultimedia);
            policyConfig.SetDefaultEndpoint(deviceId, ERole.eCommunications);
        }
    }
}
```

#### AudioSwitcher.AudioApi.CoreAudio (NuGet)

A community NuGet package that wraps both enumeration and default-device switching:
- https://github.com/xenolightning/AudioSwitcher

---

### 3.5 Standalone CLI Tools

#### NirCmd (Free, Closed Source)

**Website:** https://www.nirsoft.net/utils/nircmd.html

```bash
# Set default audio device (by friendly name)
nircmd.exe setdefaultsounddevice "Speakers"

# Set with specific role (0=Console, 1=Multimedia, 2=Communications)
nircmd.exe setdefaultsounddevice "Headphones" 1
nircmd.exe setdefaultsounddevice "Headphones" 2
```

**Pros:** Single executable, no installation needed.
**Cons:** Closed source, device must be matched by exact display name.

#### SoundVolumeView / SoundVolumeCommandLine (Free, NirSoft)

**Website:** https://www.nirsoft.net/utils/sound_volume_view.html

More powerful than NirCmd for audio control. Has both a GUI and pure command-line version (`svcl.exe`).

```bash
# Set system default device
SoundVolumeView.exe /SetDefault "Speakers" all

# Toggle between two devices
SoundVolumeView.exe /SwitchDefault "Speakers" "Headphones" all

# Set per-application audio device (Windows 10+)
SoundVolumeView.exe /SetAppDefault "Headphones\Device\Speakers\Render" 0 "firefox.exe"

# Reset app to system default
SoundVolumeView.exe /SetAppDefault DefaultRenderDevice 0 "firefox.exe"

# Volume control
SoundVolumeView.exe /SetVolume "Speakers" 75
SoundVolumeView.exe /Mute "Speakers"
SoundVolumeView.exe /Unmute "Speakers"
```

**Unique advantage:** SoundVolumeView is the only CLI tool that supports **per-application audio device routing** on Windows 10+.

#### SoundSwitch (Open Source, C#)

**Website:** https://soundswitch.aaflalo.me/
**Repository:** https://github.com/Belphemur/SoundSwitch
**Install:** `winget install SoundSwitch` or via Microsoft Store

A systray application with hotkey support and a CLI interface.

```bash
# Switch to next playback device (cycles through configured list)
SoundSwitch.CLI.exe switch --type Playback

# Switch to next recording device
SoundSwitch.CLI.exe switch --type Recording

# Microphone mute control
SoundSwitch.CLI.exe mute --toggle
SoundSwitch.CLI.exe mute --state true
SoundSwitch.CLI.exe mute --state false

# Profile management
SoundSwitch.CLI.exe profile --list
SoundSwitch.CLI.exe profile --name "Gaming Setup"
```

**Default hotkeys:** Ctrl+Alt+F11 (playback), Ctrl+Alt+F7 (recording), Ctrl+Alt+M (mic mute).

**Pros:** Open source, actively maintained, profile support, hotkeys, CLI.
**Cons:** Requires SoundSwitch service running; switches to "next" device rather than specific device.

---

## 4. Dynamic Detection of Device Changes

### IMMNotificationClient (C/C++)

The official Windows API for detecting audio device changes. You implement the `IMMNotificationClient` interface and register it with `IMMDeviceEnumerator`.

**Events you can detect:**
| Method | Trigger |
|--------|---------|
| `OnDeviceAdded(deviceId)` | New audio device connected |
| `OnDeviceRemoved(deviceId)` | Audio device disconnected |
| `OnDeviceStateChanged(deviceId, newState)` | Device enabled/disabled/unplugged |
| `OnDefaultDeviceChanged(flow, role, deviceId)` | Default device changed |
| `OnPropertyValueChanged(deviceId, key)` | Device property changed (e.g., name) |

**C++ implementation example** (from Microsoft documentation):

```cpp
class CMMNotificationClient : public IMMNotificationClient
{
    LONG _cRef;
    IMMDeviceEnumerator *_pEnumerator;

public:
    CMMNotificationClient() : _cRef(1), _pEnumerator(NULL) {}

    // IUnknown methods
    ULONG STDMETHODCALLTYPE AddRef() { return InterlockedIncrement(&_cRef); }
    ULONG STDMETHODCALLTYPE Release() {
        ULONG ulRef = InterlockedDecrement(&_cRef);
        if (0 == ulRef) delete this;
        return ulRef;
    }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID riid, VOID **ppvInterface) {
        if (IID_IUnknown == riid) { AddRef(); *ppvInterface = (IUnknown*)this; }
        else if (__uuidof(IMMNotificationClient) == riid) { AddRef(); *ppvInterface = (IMMNotificationClient*)this; }
        else { *ppvInterface = NULL; return E_NOINTERFACE; }
        return S_OK;
    }

    // Notification callbacks
    HRESULT STDMETHODCALLTYPE OnDefaultDeviceChanged(EDataFlow flow, ERole role, LPCWSTR pwstrDeviceId) {
        // Called when default device changes
        printf("Default device changed: flow=%d, role=%d, id=%S\n", flow, role, pwstrDeviceId);
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE OnDeviceAdded(LPCWSTR pwstrDeviceId) {
        printf("Device added: %S\n", pwstrDeviceId);
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE OnDeviceRemoved(LPCWSTR pwstrDeviceId) {
        printf("Device removed: %S\n", pwstrDeviceId);
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE OnDeviceStateChanged(LPCWSTR pwstrDeviceId, DWORD dwNewState) {
        printf("Device state changed: %S -> %d\n", pwstrDeviceId, dwNewState);
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE OnPropertyValueChanged(LPCWSTR pwstrDeviceId, const PROPERTYKEY key) {
        return S_OK;
    }
};

// Registration
IMMDeviceEnumerator *pEnumerator = NULL;
CoCreateInstance(__uuidof(MMDeviceEnumerator), NULL, CLSCTX_ALL,
                 __uuidof(IMMDeviceEnumerator), (void**)&pEnumerator);

CMMNotificationClient *pClient = new CMMNotificationClient();
pEnumerator->RegisterEndpointNotificationCallback(pClient);

// ... application runs ...

// Cleanup
pEnumerator->UnregisterEndpointNotificationCallback(pClient);
```

**Critical rules for IMMNotificationClient callbacks:**
1. Callback methods MUST be **non-blocking** -- never wait on synchronization objects.
2. NEVER call `RegisterEndpointNotificationCallback` or `UnregisterEndpointNotificationCallback` from within a callback (causes deadlocks).
3. NEVER release the final reference on an MMDevice API object during a callback.

### PowerShell Polling Approach

No native event-based approach in PowerShell, but you can poll:

```powershell
$lastDevice = (Get-AudioDevice -Playback).Name
while ($true) {
    Start-Sleep -Seconds 2
    $currentDevice = (Get-AudioDevice -Playback).Name
    if ($currentDevice -ne $lastDevice) {
        Write-Host "Default device changed: $lastDevice -> $currentDevice"
        $lastDevice = $currentDevice
    }
}
```

### Python Event Detection (pycaw + comtypes)

```python
# pyWinCoreAudio provides callback support for device changes
# Alternatively, you can poll with pycaw:
import time
from pycaw.pycaw import AudioUtilities

last_default = None
while True:
    speakers = AudioUtilities.GetSpeakers()
    iface = speakers.QueryInterface(comtypes.gen.MMDeviceAPI.IMMDevice)
    device_id = iface.GetId()
    if device_id != last_default:
        print(f"Default device changed to: {device_id}")
        last_default = device_id
    time.sleep(2)
```

---

## 5. Gotchas and Limitations

### The IPolicyConfig Interface is Undocumented

- Microsoft has **never officially documented** IPolicyConfig. It is reverse-engineered.
- The COM interface GUIDs have changed between Windows versions (Vista vs. 7+).
- A future Windows update could break tools that rely on it. In practice, the interface has been stable since Windows 7 through Windows 11 (as of February 2026).
- The SoundSwitch project (actively maintained, 5000+ stars) depends on it and has dealt with any breakage promptly.

### Per-Application Audio Routing Has No Public API

- Windows 10+ has "App volume and device preferences" in Settings, but there is **no documented API** to control per-app audio routing.
- The only tool that exposes this via CLI is **SoundVolumeView** (`/SetAppDefault` command).
- Third-party solutions (Voicemeeter, Virtual Audio Cable, Audio Router) exist but require their own drivers.

### Automatic Stream Routing Behavior

- Starting with Windows 7, when the default device changes, applications using the **default device endpoint** will have their streams automatically rerouted to the new device.
- This only works for streams opened via `IMMDeviceEnumerator::GetDefaultAudioEndpoint`. If an application hardcodes a specific device, its stream will NOT be rerouted.
- Applications using low-level WASAPI (prior to Windows 10 1607) must implement their own stream routing.
- After Windows 10 1607, WASAPI apps can opt into automatic routing with the `DEVINTERFACE_AUDIO_RENDER` / `DEVINTERFACE_AUDIO_CAPTURE` GUIDs.

### Permissions

- Switching the default audio device via IPolicyConfig does **not** require administrator privileges in normal circumstances.
- Installing PowerShell modules (`Install-Module`) may require admin for system-wide installation, but `-Scope CurrentUser` works without admin.
- No special permissions are needed for NirCmd or SoundVolumeView.

### Windows Version Differences

| Feature | Vista | 7 | 8/8.1 | 10 | 11 |
|---------|-------|---|-------|-----|-----|
| MMDevice enumeration | Yes | Yes | Yes | Yes | Yes |
| IPolicyConfigVista | Yes | Yes | Yes | Yes | Yes |
| IPolicyConfig (Win7+) | No | Yes | Yes | Yes | Yes |
| Automatic stream routing | No | Yes | Yes | Yes | Yes |
| Per-app device routing (UI) | No | No | No | Yes | Yes |
| Per-app device routing (API) | No | No | No | No | No |
| IMMNotificationClient | Yes | Yes | Yes | Yes | Yes |

### Device Name Instability

- Device **friendly names** can change (e.g., after a driver update or when multiple identical devices are connected).
- Always prefer **endpoint ID strings** for reliable device identification.
- Endpoint IDs survive reboots but may change if the device is plugged into a different port.

### Communication Device vs. Default Device

- Windows has separate concepts for "Default Device" and "Default Communication Device."
- When switching, you typically want to set **all three roles** (eConsole, eMultimedia, eCommunications) unless you have a specific reason to keep them separate (e.g., headset for calls, speakers for music).

---

## 6. Recommendation Summary

### For Quick Scripting (Best Starting Point)

**PowerShell + AudioDeviceCmdlets** is the fastest path:
- One-line install
- Simple, readable commands
- Works from WSL via `powershell.exe -Command "..."`
- No compilation needed

### For Python Applications

Use **pycaw** for enumeration and volume control, combined with **direct comtypes IPolicyConfig calls** for switching the default device. Alternatively, use **pyaudiodevice** which wraps both into a single package.

### For C#/.NET Applications

Use **NAudio** for enumeration + the **IPolicyConfig COM interop** pattern shown above for switching. Or use the **AudioSwitcher.AudioApi.CoreAudio** NuGet package.

### For CLI Automation

- **NirCmd** for simple default-device switching
- **SoundVolumeView/svcl** for advanced features including per-app routing
- **SoundSwitch CLI** for hotkey-driven cycling through devices

### For Event-Driven Applications

Implement `IMMNotificationClient` via the Core Audio API (C/C++/C#) or use **pyWinCoreAudio** for Python callback support.

---

## Sources

- [AudioDeviceCmdlets - GitHub](https://github.com/frgnca/AudioDeviceCmdlets)
- [AudioDeviceCmdlets - PowerShell Gallery](https://www.powershellgallery.com/packages/AudioDeviceCmdlets/3.1.0.2)
- [pycaw - Python Core Audio Windows](https://github.com/AndreMiras/pycaw)
- [pyaudiodevice - PyPI](https://pypi.org/project/pyaudiodevice/)
- [pyWinCoreAudio - GitHub](https://github.com/kdschlosser/pyWinCoreAudio)
- [PolicyConfig.h Gist](https://gist.github.com/VonLatvala/021b048297973a3d47ed7e39dfe2adf0)
- [AudioEndPointLibrary PolicyConfig.h (SoundSwitch)](https://github.com/Belphemur/AudioEndPointLibrary/blob/master/DefSound/PolicyConfig.h)
- [SoundSwitch](https://github.com/Belphemur/SoundSwitch)
- [SoundSwitch CLI README](https://github.com/Belphemur/SoundSwitch/blob/dev/SoundSwitch.CLI/README.md)
- [NirCmd setdefaultsounddevice](https://nircmd.nirsoft.net/setdefaultsounddevice.html)
- [SoundVolumeView](https://www.nirsoft.net/utils/sound_volume_view.html)
- [Set default audio device from command line (NirSoft)](https://www.nirsoft.net/articles/set_default_audio_device_command_line.html)
- [Per-app audio device routing (NirSoft blog)](https://blog.nirsoft.net/2020/05/29/set-default-audio-device-of-specific-application-from-command-line-on-windows-10/)
- [NAudio Enumerate Output Devices](https://github.com/naudio/NAudio/blob/master/Docs/EnumerateOutputDevices.md)
- [IMMNotificationClient - Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/api/mmdeviceapi/nn-mmdeviceapi-immnotificationclient)
- [IMMDeviceEnumerator::RegisterEndpointNotificationCallback - Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/api/mmdeviceapi/nf-mmdeviceapi-immdeviceenumerator-registerendpointnotificationcallback)
- [Device Events (Core Audio APIs) - Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/coreaudio/device-events)
- [Automatic Stream Routing - Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/coreaudio/automatic-stream-routing)
- [Stream Routing Implementation - Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/coreaudio/stream-routing-implementation-considerations)
- [Core Audio Interfaces - Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/coreaudio/core-audio-interfaces)
- [Programmatically switch audio devices (blog)](https://andreypicado.com/programmatically-switch-between-audio-devices-in-windows/)
- [DefaultAudioChanger - GitHub](https://github.com/sgiurgiu/DefaultAudioChanger)
- [AudioSwitcher - GitHub](https://github.com/marcjoha/AudioSwitcher)
- [Python enumerate audio devices gist](https://gist.github.com/papr/208eb04d7b73ef63607f8ff92c3a34ac)
- [CodeMachine - How Windows Sets Default Audio Device](https://codemachine.com/articles/how_windows_sets_default_audio_device.html)
