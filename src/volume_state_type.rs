use windows::Win32::Media::Audio::AUDIO_VOLUME_NOTIFICATION_DATA;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;

/// An update about the state of the device.  
/// Muted/unmuted? New volume?  
/// These are the questions this provides answers to

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct VolumeState {
    pub is_muted: bool,
    pub volume: f32,
}

impl TryFrom<&IAudioEndpointVolume> for VolumeState {
    type Error = windows_core::Error;

    fn try_from(endpoint_volume: &IAudioEndpointVolume) -> Result<Self, Self::Error> {
        let volume = unsafe { endpoint_volume.GetMasterVolumeLevelScalar() }?;
        let is_muted = unsafe { endpoint_volume.GetMute() }?.into();

        Ok(VolumeState { is_muted, volume })
    }
}

impl From<&AUDIO_VOLUME_NOTIFICATION_DATA> for VolumeState {
    fn from(data: &AUDIO_VOLUME_NOTIFICATION_DATA) -> Self {
        let volume = data.fMasterVolume;
        let is_muted = data.bMuted.into();

        VolumeState { is_muted, volume }
    }
}
