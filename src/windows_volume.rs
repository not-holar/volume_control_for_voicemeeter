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
    pub inner: Arc<VolumeObserverInner>,
}

impl VolumeObserver {
    /// Observe volume changes of the device whose name contains [`name`]
    pub fn from_device_name(name: &str) -> Result<Self, String> {
        let device = Self::find_system_device_by_name(name)
            .map_err(|err| format!("Failed to access Windows devices: {err}"))?
            .ok_or(format!(
                "Couldn't find a windows device with \"{name}\" in it's name."
            ))?;

        let (tx, _) = tokio::sync::broadcast::channel(2);

        let inner = VolumeObserverInner::new(tx.clone(), &device)?;

        Ok(Self {
            inner: inner.into(),
        })
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<f32> {
        self.inner.tx.subscribe()
    }

    fn find_system_device_by_name(name: &str) -> Result<Option<IMMDevice>, String> {
        Ok(Self::all_endpoints()?.find_map(|(device_name, endpoint)| {
            device_name?
                .to_lowercase()
                .contains(name)
                .then_some(endpoint)
        }))
    }

    fn all_endpoints() -> Result<impl Iterator<Item = (Option<String>, IMMDevice)>, String> {
        let endpoints = unsafe {
            CoCreateInstance::<_, IMMDeviceEnumerator>(
                &MMDeviceEnumerator,
                None,
                CLSCTX_INPROC_SERVER,
            ) // Create device enumerator
            .map_err(|err| format!("Failed to create DeviceEnumerator: {}", err))?
            .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
            .map_err(|err| format!("Failed to Enumerate Audio Endpoints: {}", err))?
        };

        Ok(unsafe {
            (0..(endpoints
                .GetCount()
                .map_err(|_| "Couldn't count endpoints.")?))
                .filter_map(move |i| endpoints.Item(i).ok())
                .map(|endpoint| (Self::endpoint_name(&endpoint), endpoint))
                .into_iter()
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

#[derive(Debug, Clone)]
pub struct VolumeObserverInner {
    pub tx: tokio::sync::broadcast::Sender<f32>,
    _keepalive: (IAudioEndpointVolumeCallback, IAudioEndpointVolume),
}

impl VolumeObserverInner {
    pub fn new(
        tx: tokio::sync::broadcast::Sender<f32>,
        device: &IMMDevice,
    ) -> Result<Self, String> {
        // Don't drop this!
        let endpoint_volume =
            unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_INPROC_SERVER, None) }
                .map_err(|err| format!("Failed to activate device: {err}"))?;

        // Don't drop this either!
        let callback = IAudioEndpointVolumeCallback::from(Callback { tx: tx.clone() });

        unsafe { endpoint_volume.RegisterControlChangeNotify(&callback) }
            .map_err(|err| format!("Couldn't register volume callback: {err:?}"))?;

        Ok(Self {
            tx,
            _keepalive: (callback, endpoint_volume),
        })
    }
}

#[derive(Debug, Clone)]
#[windows::core::implement(IAudioEndpointVolumeCallback)]
struct Callback {
    pub tx: tokio::sync::broadcast::Sender<f32>,
}

#[allow(non_snake_case)]
impl IAudioEndpointVolumeCallback_Impl for Callback {
    fn OnNotify(&self, data: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        let _ = self.tx.send(unsafe { &*data }.fMasterVolume);
        Ok(())
    }
}

/// Initialize Win32's COM library. Things break without this step.
pub fn initialize_com() -> ::windows::core::Result<()> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
}
