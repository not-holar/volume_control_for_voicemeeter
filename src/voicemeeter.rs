use crate::volume_state_type::VolumeState;

use ::voicemeeter::interface::parameters::Strip;
use anyhow::Context;

/// The link between the app and Voicemeeter.
///
/// Logging in to the Remote API works whether or not Voicemeeter is running;
/// the API starts answering as soon as Voicemeeter (re)starts.
#[derive(Debug)]
pub struct Link {
    remote: ::voicemeeter::VoicemeeterRemote,
}

impl Link {
    pub fn new() -> Result<Self, ::voicemeeter::interface::InitializationError> {
        ::voicemeeter::VoicemeeterRemote::new().map(|remote| Self { remote })
    }

    fn virtual_inputs(&self) -> impl Iterator<Item = Strip<'_>> {
        let parameters = self.remote.parameters();

        (0..)
            .map_while(move |index| parameters.strip(index).ok())
            .filter(Strip::is_virtual)
    }

    pub fn is_connected(&self) -> bool {
        self.remote.get_voicemeeter_type().is_ok() && self.remote.is_parameters_dirty().is_ok()
    }

    pub fn program_name(&self) -> String {
        match self.remote.get_voicemeeter_type() {
            Ok(program) => format!("{program:?}"),
            Err(_) => "Voicemeeter".to_string(),
        }
    }

    /// Apply the volume state to the first virtual input strip (the one fed by
    /// the "VoiceMeeter Input" device).
    pub fn apply(&mut self, state: VolumeState, gain_db: f32) -> anyhow::Result<()> {
        // Refresh the cached program type so that the right strips are found,
        // the user may have switched between Voicemeeter editions.
        self.remote
            .update_program()
            .context("Couldn't read the Voicemeeter edition")?;

        let strip = self
            .virtual_inputs()
            .next()
            .context("Couldn't find a virtual input strip in Voicemeeter")?;

        strip
            .gain()
            .set(gain_db)
            .context("Couldn't set Voicemeeter strip gain")?;

        strip
            .mute()
            .set(state.is_muted)
            .context("Couldn't set Voicemeeter strip mute")?;

        Ok(())
    }
}
