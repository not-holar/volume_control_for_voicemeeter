use anyhow::Context;
use std::sync::Arc;

use windows::Win32::{
    Devices::FunctionDiscovery::PKEY_Device_DeviceDesc,
    Media::Audio::{
        eRender,
        Endpoints::{
            IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
        },
        IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, AUDIO_VOLUME_NOTIFICATION_DATA,
        DEVICE_STATE_ACTIVE,
    },
    System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READ,
    },
};

use crate::volume_curves;

#[derive(Debug, Clone)]
pub struct VolumeObserver {
    inner: Arc<VolumeObserverInner>,
}

impl VolumeObserver {
    /// Observe volume changes of the device whose name contains [`name`]
    pub fn from_device_name(name: &str) -> anyhow::Result<Self> {
        let device = Self::find_system_device_by_name(name)
            .context("Failed to access Windows devices")?
            .with_context(|| {
                format!("Couldn't find a windows device with \"{name}\" in it's name.")
            })?;

        let inner = VolumeObserverInner::new(&device)?;

        Ok(Self {
            inner: inner.into(),
        })
    }

    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<Option<f32>> {
        self.inner.rx.clone()
    }

    fn find_system_device_by_name(name: &str) -> anyhow::Result<Option<IMMDevice>> {
        Ok(Self::all_endpoints()?.find_map(|(device_name, endpoint)| {
            let device_name = device_name?;

            let matches = device_name.to_lowercase().contains(name);

            println!(
                "{} {device_name}",
                match matches {
                    false => "❌\t",
                    true => "✔\t",
                }
            );

            matches.then_some(endpoint)
        }))
    }

    fn all_endpoints() -> anyhow::Result<impl Iterator<Item = (Option<String>, IMMDevice)>> {
        let endpoints = unsafe {
            CoCreateInstance::<_, IMMDeviceEnumerator>(
                &MMDeviceEnumerator,
                None,
                CLSCTX_INPROC_SERVER,
            ) // Create device enumerator
            .context("Failed to create DeviceEnumerator")?
            .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
            .context("Failed to Enumerate Audio Endpoints")?
        };

        Ok(unsafe {
            (0..(endpoints.GetCount().context("Couldn't count endpoints.")?))
                .filter_map(move |i| endpoints.Item(i).ok())
                .map(|endpoint| (Self::endpoint_name(&endpoint), endpoint))
        })
    }

    fn endpoint_name(endpoint: &IMMDevice) -> Option<String> {
        Some(unsafe {
            endpoint
                .OpenPropertyStore(STGM_READ)
                .ok()?
                // .GetValue(&PKEY_Device_FriendlyName)
                .GetValue(&PKEY_Device_DeviceDesc)
                .ok()?
                .to_string()
        })
    }
}

#[derive(Debug)]
struct VolumeObserverInner {
    pub rx: tokio::sync::watch::Receiver<Option<f32>>,
    _keepalive: (IAudioEndpointVolumeCallback, IAudioEndpointVolume),
}

impl VolumeObserverInner {
    pub fn new(device: &IMMDevice) -> anyhow::Result<Self> {
        // Don't drop this!
        let endpoint_volume =
            unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_INPROC_SERVER, None) }
                .context("Failed to activate device")?;

        let (tx, rx) = tokio::sync::watch::channel(
            unsafe { endpoint_volume.GetMasterVolumeLevelScalar() }.ok(),
        );

        // Don't drop this either!
        let callback = IAudioEndpointVolumeCallback::from(Callback { tx });

        unsafe { endpoint_volume.RegisterControlChangeNotify(&callback) }
            .context("Couldn't register volume callback")?;

        Ok(Self {
            rx,
            _keepalive: (callback, endpoint_volume),
        })
    }
}

#[derive(Debug)]
#[windows::core::implement(IAudioEndpointVolumeCallback)]
struct Callback {
    pub tx: tokio::sync::watch::Sender<Option<f32>>,
}

#[allow(non_snake_case)]
impl IAudioEndpointVolumeCallback_Impl for Callback {
    fn OnNotify(&self, data: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        self.tx.send_if_modified(|x| {
            let volume = unsafe { &*data }.fMasterVolume;
            let muted: bool = unsafe { &*data }.bMuted.into();
            let valid = Some(muted).is_some() && Some(volume).is_some();
            if valid {
                let actualVolume = if muted { 0.0 } else {volume};
                
                if x.expect("Value must be valid") != actualVolume{
                    x.replace(volume_curves::ease_out_expo(actualVolume));
                    true
                }
                else {
                    false
                }
            }
            else {
                false
            }
        });
        // .expect("IAudioEndpointVolumeCallback_Impl send error");
        Ok(())
    }
}

/// Initialize Win32's COM library. Things break without this step.
pub fn initialize_com() -> ::windows::core::Result<()> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok()
}
