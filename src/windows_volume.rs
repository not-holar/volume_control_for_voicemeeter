use anyhow::Context;
use std::sync::Arc;

use windows::Win32::{
    Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
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

    pub fn subscribe(&self) -> smol::channel::Receiver<f32> {
        self.inner.rx.clone()
    }

    fn find_system_device_by_name(name: &str) -> anyhow::Result<Option<IMMDevice>> {
        Ok(Self::all_endpoints()?.find_map(|(device_name, endpoint)| {
            device_name?
                .to_lowercase()
                .contains(name)
                .then_some(endpoint)
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
                .GetValue(&PKEY_Device_FriendlyName)
                .ok()?
                .Anonymous
                .Anonymous
                .Anonymous
                .pwszVal
                .to_string()
                .ok()?
        })
    }
}

#[derive(Debug)]
struct VolumeObserverInner {
    pub rx: smol::channel::Receiver<f32>,
    _keepalive: (IAudioEndpointVolumeCallback, IAudioEndpointVolume),
}

impl VolumeObserverInner {
    pub fn new(device: &IMMDevice) -> anyhow::Result<Self> {
        // Don't drop this!
        let endpoint_volume =
            unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_INPROC_SERVER, None) }
                .context("Failed to activate device")?;

        let (tx, rx) = smol::channel::unbounded();

        if let Ok(initial_volume) = unsafe { endpoint_volume.GetMasterVolumeLevelScalar() } {
            tx.send_blocking(initial_volume)
                .context("Stream error when sending initial volume")?;
        }

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
    pub tx: smol::channel::Sender<f32>,
}

#[allow(non_snake_case)]
impl IAudioEndpointVolumeCallback_Impl for Callback {
    fn OnNotify(&self, data: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        self.tx
            .send_blocking(unsafe { &*data }.fMasterVolume)
            .expect("IAudioEndpointVolumeCallback_Impl send error");
        Ok(())
    }
}

/// Initialize Win32's COM library. Things break without this step.
pub fn initialize_com() -> ::windows::core::Result<()> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
}
