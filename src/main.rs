// mod eco_mode;
mod voicemeeter;

use lerp::Lerp;

use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolumeCallback;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolumeCallback_Impl;

use windows::Win32::Media::Audio::{
    eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, AUDIO_VOLUME_NOTIFICATION_DATA,
    DEVICE_STATE_ACTIVE,
};

use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READ,
};

#[windows::core::implement(IAudioEndpointVolumeCallback)]
pub struct VolumeObserver {
    tx: tokio::sync::broadcast::Sender<f32>,
}

#[allow(non_snake_case)]
impl IAudioEndpointVolumeCallback_Impl for VolumeObserver {
    fn OnNotify(&self, data: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        let _ = self.tx.send(unsafe { &*data }.fMasterVolume);
        Ok(())
    }
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

fn all_endpoints() -> Result<impl Iterator<Item = (Option<String>, IMMDevice)>, String> {
    let endpoints = unsafe {
        CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)
            .map_err(|err| format!("Failed to create DeviceEnumerator: {}", err))?
            .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
            .map_err(|err| format!("Failed to Enumerate Audio Endpoints: {}", err))?
    };

    Ok(unsafe {
        (0..(endpoints
            .GetCount()
            .map_err(|_| "Couldn't count endpoints.")?))
            .filter_map(move |i| endpoints.Item(i).ok())
            .map(|endpoint| (endpoint_name(&endpoint), endpoint))
            .into_iter()
    })
}

fn system_voicemeeter_device() -> Result<Option<IMMDevice>, String> {
    Ok(all_endpoints()?.find_map(|(name, endpoint)| {
        name?
            .to_lowercase()
            .contains("voicemeeter vaio")
            .then_some(endpoint)
    }))
}

#[tokio::main]
async fn main() {
    listen().await.unwrap_or_else(|err| {
        println!("{err}");

        println!("\nPress ENTER to continue...");
        std::io::stdin().lines().next();
    });
}

async fn listen() -> Result<(), String> {
    // Initialize Win32's COM interface. Things break without this step.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .map_err(|err| format!("CoInitializeEx failed: {err}"))?;

    // eco_mode::set_eco_mode_for_current_process()
    //     .unwrap_or_else(|err| println!("Failed to set Process mode to Eco: {}", err));

    let voicemeeter_link = voicemeeter::Link::new()
        .map_err(|err| match &err {
            voicemeeter::LinkCreationError::RemoteInit(inner) => match inner {
                ::voicemeeter::interface::InitializationError::LoginError(_) => {
                    format!("{err:?}\nIs Voicemeeter running?")
                }
                _ => format!("{err:?}"),
            },
        })
        .map_err(|err| format!("Failed to connect to Voicemeeter: {err}"))?;

    let voicemeeter_gain_parameter = voicemeeter_link.gain_parameter(&voicemeeter_link.virtual_inputs().nth(0)
    	.ok_or("There should absolutely be at least one Virtual Input in any Voicemeeter edition but it's not there 🤷.")?);

    let device = system_voicemeeter_device()
        .map_err(|err| format!("Failed to access Windows devices: {err}"))?
        .ok_or("Couldn't find Voicemeeter's virtual input (VAIO) device.")?;

    let activation_handle =
        unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_INPROC_SERVER, None) }
            .map_err(|err| format!("Failed to activate device: {err}"))?;

    let (tx, mut rx) = tokio::sync::broadcast::channel(1);

    // Don't drop this!
    let callback_handle = IAudioEndpointVolumeCallback::from(VolumeObserver { tx });

    unsafe { activation_handle.RegisterControlChangeNotify(&callback_handle) }
        .map_err(|err| format!("Couldn't register volume callback: {err:?}"))?;

    loop {
        let t = rx
            .recv()
            .await
            .map_err(|err| format!("Stream error: {err:?}"))?;

        let gain = (-60.0).lerp(0.0, t);

        voicemeeter_gain_parameter
            .set(gain)
            .unwrap_or_else(|err| println!("Couldn't set slider value: {err:?}"))
    }
}
