use crate::volume_state_type::VolumeState;

use anyhow::Context;
use smol::channel::Sender;
use windows::Win32::{
    Devices::FunctionDiscovery::PKEY_Device_DeviceDesc,
    Foundation::PROPERTYKEY,
    Media::Audio::{
        AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE, DEVICE_STATE_ACTIVE, EDataFlow, ERole,
        Endpoints::{
            IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
        },
        IMMDevice, IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl,
        MMDeviceEnumerator, eRender,
    },
    System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, STGM_READ,
    },
};
use windows_core::PCWSTR;

/// Initialize Win32's COM library. Things break without this step.
///
/// Multithreaded apartment: the main thread has no message pump, and all
/// audio notifications arrive on threads owned by the audio service anyway.
pub fn initialize_com() -> windows::core::Result<()> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()
}

/// Watches the system for audio devices being added, removed, enabled or disabled.
///
/// Voicemeeter's virtual device can be re-created by Windows (driver/engine restarts,
/// sleep/resume, format changes...), which silently invalidates any volume callback
/// registered on the old instance. These notifications tell us when to look again.
pub struct DeviceWatcher {
    enumerator: IMMDeviceEnumerator,
    client: IMMNotificationClient,
}

impl DeviceWatcher {
    pub fn new(on_change: Sender<()>) -> anyhow::Result<Self> {
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER) }
                .context("Failed to create the audio device enumerator")?;

        let client: IMMNotificationClient = DeviceNotifications { on_change }.into();

        unsafe { enumerator.RegisterEndpointNotificationCallback(&client) }
            .context("Couldn't register for audio device notifications")?;

        Ok(Self { enumerator, client })
    }

    /// Start observing the first active playback device whose description contains
    /// `lowercase_name`. `Ok(None)` means no such device is currently active.
    pub fn observe(
        &self,
        lowercase_name: &str,
        on_change: Sender<()>,
    ) -> anyhow::Result<Option<VolumeObserver>> {
        let endpoints = unsafe { self.enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) }
            .context("Failed to enumerate audio endpoints")?;
        let count = unsafe { endpoints.GetCount() }.context("Couldn't count audio endpoints")?;

        for index in 0..count {
            let Ok(device) = (unsafe { endpoints.Item(index) }) else {
                continue;
            };
            let Some(name) = endpoint_name(&device) else {
                continue;
            };
            if name.to_lowercase().contains(lowercase_name) {
                return VolumeObserver::new(&device, name, on_change).map(Some);
            }
        }

        Ok(None)
    }
}

impl Drop for DeviceWatcher {
    fn drop(&mut self) {
        let _ = unsafe {
            self.enumerator
                .UnregisterEndpointNotificationCallback(&self.client)
        };
    }
}

fn endpoint_name(endpoint: &IMMDevice) -> Option<String> {
    Some(unsafe {
        endpoint
            .OpenPropertyStore(STGM_READ)
            .ok()?
            .GetValue(&PKEY_Device_DeviceDesc)
            .ok()?
            .to_string()
    })
}

/// Signals `on_change` whenever the volume or mute state of one device changes.
/// The callback is unregistered when this is dropped.
pub struct VolumeObserver {
    name: String,
    endpoint_volume: IAudioEndpointVolume,
    callback: IAudioEndpointVolumeCallback,
}

impl VolumeObserver {
    fn new(device: &IMMDevice, name: String, on_change: Sender<()>) -> anyhow::Result<Self> {
        let endpoint_volume: IAudioEndpointVolume =
            unsafe { device.Activate(CLSCTX_INPROC_SERVER, None) }
                .context("Failed to activate the audio device")?;

        let callback: IAudioEndpointVolumeCallback = VolumeCallback { on_change }.into();

        unsafe { endpoint_volume.RegisterControlChangeNotify(&callback) }
            .context("Couldn't register volume callback")?;

        Ok(Self {
            name,
            endpoint_volume,
            callback,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Current volume and mute state. Fails once the device has gone away.
    pub fn state(&self) -> windows_core::Result<VolumeState> {
        VolumeState::try_from(&self.endpoint_volume)
    }
}

impl Drop for VolumeObserver {
    fn drop(&mut self) {
        let _ = unsafe {
            self.endpoint_volume
                .UnregisterControlChangeNotify(&self.callback)
        };
    }
}

// The COM callbacks below run on audio service threads. They must return quickly
// and must not call back into COM, so all they do is wake up the main loop.

#[windows::core::implement(IAudioEndpointVolumeCallback)]
struct VolumeCallback {
    on_change: Sender<()>,
}

#[allow(non_snake_case)]
impl IAudioEndpointVolumeCallback_Impl for VolumeCallback_Impl {
    fn OnNotify(&self, _data: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        let _ = self.on_change.try_send(());
        Ok(())
    }
}

#[windows::core::implement(IMMNotificationClient)]
struct DeviceNotifications {
    on_change: Sender<()>,
}

#[allow(non_snake_case)]
impl IMMNotificationClient_Impl for DeviceNotifications_Impl {
    fn OnDeviceStateChanged(&self, _id: &PCWSTR, _state: DEVICE_STATE) -> windows::core::Result<()> {
        let _ = self.on_change.try_send(());
        Ok(())
    }

    fn OnDeviceAdded(&self, _id: &PCWSTR) -> windows::core::Result<()> {
        let _ = self.on_change.try_send(());
        Ok(())
    }

    fn OnDeviceRemoved(&self, _id: &PCWSTR) -> windows::core::Result<()> {
        let _ = self.on_change.try_send(());
        Ok(())
    }

    fn OnDefaultDeviceChanged(
        &self,
        _flow: EDataFlow,
        _role: ERole,
        _id: &PCWSTR,
    ) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnPropertyValueChanged(&self, _id: &PCWSTR, _key: &PROPERTYKEY) -> windows::core::Result<()> {
        Ok(())
    }
}
