// mod eco_mode;

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

fn voicemeeter_nth_virtual_input_gain_parameter(
    remote: &voicemeeter::VoicemeeterRemote,
    n: usize,
) -> Option<voicemeeter::types::ParameterName> {
    let parameters = remote.parameters();

    Some(
        (0..)
            .map_while(|index| parameters.strip(index).ok()) // take all existing strips
            .filter(|strip| strip.is_virtual()) // leave only virtual ones
            .nth(n)? // take the n-th one
            .param("Gain")
            .into(),
    )
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
    // Initialize Win32's COM interface. Things break without this step.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .unwrap_or_else(|err| println!("CoInitializeEx failed: {}", err));

    // eco_mode::set_eco_mode_for_current_process()
    //     .unwrap_or_else(|err| println!("Failed to set Process mode to Eco: {}", err));

    let remote = voicemeeter::VoicemeeterRemote::new()
        .expect("Couldn't connect to Voicemeeter, make sure it is running.");

    let voicemeeter_gain = voicemeeter_nth_virtual_input_gain_parameter(&remote, 0)
    	.expect("There should absolutely be at least one Virtual Input in any Voicemeeter edition but it's not there 🤷.");

    let device = system_voicemeeter_device()
        .unwrap()
        .expect("Couldn't find Voicemeeter's virtual input (VAIO) device.");

    let activation_handle =
        unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_INPROC_SERVER, None) }
            .expect("Failed to activate device.");

    let (tx, mut rx) = tokio::sync::broadcast::channel(1);

    // Don't drop this!
    let callback_handle = IAudioEndpointVolumeCallback::from(VolumeObserver { tx });

    unsafe { activation_handle.RegisterControlChangeNotify(&callback_handle) }
        .unwrap_or_else(|err| println!("Couldn't register volume callback: {:?}", err));

    loop {
        rx.recv()
            .await
            .ok()
            .map(|t| (-60.0).lerp(0.0, t))
            .and_then(|gain| {
                Some(
                    remote
                        .set_parameter_float(&voicemeeter_gain, gain)
                        .unwrap_or_else(|err| println!("Couldn't set slider value: {err:?}")),
                )
            });
    }
}
