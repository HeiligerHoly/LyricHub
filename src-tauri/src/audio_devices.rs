//! Default audio output and its offset (PLAN.md 5.7). The user listens over a cable, so this
//! stays plain: a thread polls the default render device every 2 s through Core Audio, and
//! every device keeps its own offset in lyrichub.db. A device seen for the first time gets a
//! starting value (Bluetooth +150 ms, else 0) and the overlay shows a short notice.

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::Devices::FunctionDiscovery::{PKEY_Device_EnumeratorName, PKEY_Device_FriendlyName};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender};
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToBSTR};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, STGM_READ};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;

use crate::app_state;
use crate::ipc::events;

const POLL: Duration = Duration::from_secs(2);
const BLUETOOTH_START_MS: i32 = 150;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub offset_ms: i32,
    /// The default output right now.
    pub current: bool,
    /// Only known for the current device.
    pub bluetooth: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct Endpoint {
    id: String,
    name: String,
    bluetooth: bool,
}

#[derive(Default)]
pub struct AudioDeviceState {
    current: Mutex<Option<Endpoint>>,
}

#[derive(Serialize, Clone)]
struct Notice {
    text: String,
}

pub fn spawn_watcher(app: AppHandle) {
    std::thread::Builder::new()
        .name("audio-devices".into())
        .spawn(move || {
            // SAFETY: COM for this thread only; MTA, nothing apartment-bound crosses threads.
            let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            // SAFETY: standard Core Audio enumerator creation.
            let enumerator: IMMDeviceEnumerator = match unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) } {
                Ok(e) => e,
                Err(e) => {
                    tracing::error!(error = %e, "audio device enumerator unavailable, device offsets stay 0");
                    return;
                }
            };
            loop {
                match read_default(&enumerator) {
                    Ok(endpoint) => {
                        let changed = app.state::<AudioDeviceState>().current.lock().unwrap().as_ref() != Some(&endpoint);
                        if changed {
                            on_change(&app, endpoint);
                        }
                    }
                    Err(e) => tracing::trace!(error = %e, "no default audio output"),
                }
                std::thread::sleep(POLL);
            }
        })
        .expect("failed to spawn audio device thread");
}

fn read_default(enumerator: &IMMDeviceEnumerator) -> windows::core::Result<Endpoint> {
    // SAFETY: Core Audio calls on objects owned by this thread; the id string is freed below.
    unsafe {
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let raw = device.GetId()?;
        let id = raw.to_string().unwrap_or_default();
        CoTaskMemFree(Some(raw.0 as *const _));
        let store = device.OpenPropertyStore(STGM_READ)?;
        let name = prop_string(&store, &PKEY_Device_FriendlyName).unwrap_or_else(|| id.clone());
        let enumerator_name = prop_string(&store, &PKEY_Device_EnumeratorName).unwrap_or_default().to_uppercase();
        let bluetooth = enumerator_name.starts_with("BTH") || name.to_lowercase().contains("bluetooth");
        Ok(Endpoint { id, name, bluetooth })
    }
}

fn prop_string(store: &IPropertyStore, key: &PROPERTYKEY) -> Option<String> {
    // SAFETY: the PROPVARIANT is read once and cleared.
    unsafe {
        let mut value = store.GetValue(key).ok()?;
        let text = PropVariantToBSTR(&value).ok().map(|b| b.to_string());
        let _ = PropVariantClear(&mut value);
        text.filter(|t| !t.is_empty())
    }
}

fn on_change(app: &AppHandle, endpoint: Endpoint) {
    let known = app_state::with_offset_store(app, |db| db.device_offset(&endpoint.id)).flatten();
    let first_device_ever = app_state::with_offset_store(app, |db| Ok(db.devices()?.is_empty())).unwrap_or(false);
    let offset = match known {
        Some(ms) => {
            app_state::with_offset_store(app, |db| db.set_device_offset(&endpoint.id, &endpoint.name, ms));
            ms
        }
        None => {
            let start = if endpoint.bluetooth { BLUETOOTH_START_MS } else { 0 };
            app_state::with_offset_store(app, |db| db.set_device_offset(&endpoint.id, &endpoint.name, start));
            // The very first device on a fresh install is the normal output, no reason to ask.
            if !first_device_ever {
                let text = format!("Neues Audiogerät: {}. Sync prüfen unter Einstellungen → Synchronisation", endpoint.name);
                if let Err(e) = app.emit(events::NOTICE, Notice { text }) {
                    tracing::warn!(error = %e, "could not emit notice");
                }
            }
            start
        }
    };
    tracing::info!(name = %endpoint.name, bluetooth = endpoint.bluetooth, new = known.is_none(), offset_ms = offset, "audio output");
    *app.state::<AudioDeviceState>().current.lock().unwrap() = Some(endpoint);
    app_state::set_device_offset_ms(app, offset);
}

/// Known devices with their offsets; the current one first.
pub fn list(app: &AppHandle) -> Vec<AudioDevice> {
    let current = app.state::<AudioDeviceState>().current.lock().unwrap().clone();
    let mut devices: Vec<AudioDevice> = app_state::with_offset_store(app, |db| db.devices())
        .unwrap_or_default()
        .into_iter()
        .map(|(id, name, offset_ms)| {
            let is_current = current.as_ref().is_some_and(|c| c.id == id);
            AudioDevice { bluetooth: is_current && current.as_ref().is_some_and(|c| c.bluetooth), current: is_current, id, name, offset_ms }
        })
        .collect();
    devices.sort_by_key(|d| !d.current);
    devices
}

pub fn current_id(app: &AppHandle) -> Option<String> {
    app.state::<AudioDeviceState>().current.lock().unwrap().as_ref().map(|c| c.id.clone())
}

/// Stores a device offset; applies at once if it is the current output.
pub fn set_offset(app: &AppHandle, id: &str, ms: i32) {
    let ms = ms.clamp(-crate::sync::offsets::LIMIT_MS, crate::sync::offsets::LIMIT_MS);
    let name = list(app).into_iter().find(|d| d.id == id).map(|d| d.name).unwrap_or_default();
    app_state::with_offset_store(app, |db| db.set_device_offset(id, &name, ms));
    tracing::info!(device = %name, offset_ms = ms, "device offset set");
    if current_id(app).as_deref() == Some(id) {
        app_state::set_device_offset_ms(app, ms);
    }
}
