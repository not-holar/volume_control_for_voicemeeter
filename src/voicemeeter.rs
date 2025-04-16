#[derive(Debug)]
pub struct Link {
    pub remote: ::voicemeeter::VoicemeeterRemote,
}

/// The link between the app and Voicemeeter, at least one copy of the first one you call must remain alive otherwise things break!
impl Link {
    pub fn new() -> Result<Self, LinkCreationError> {
        ::voicemeeter::VoicemeeterRemote::new()
            .map_err(LinkCreationError::RemoteInit)
            .map(|remote| Self { remote })
    }
}

/// Error that can arise when creating [`VoicemeeterLink`].
#[derive(Debug, thiserror::Error)]
pub enum LinkCreationError {
    #[error("{0}")]
    RemoteInit(#[from] ::voicemeeter::interface::InitializationError),
}

impl Link {
    pub fn strips(&self) -> impl Iterator<Item = ::voicemeeter::interface::parameters::Strip> {
        let parameters = self.remote.parameters();

        (0..).map_while(move |index| parameters.strip(index).ok())
    }

    pub fn virtual_inputs(
        &self,
    ) -> impl Iterator<Item = ::voicemeeter::interface::parameters::Strip> {
        self.strips() // take all existing strips
            .filter(::voicemeeter::interface::parameters::Strip::is_virtual) // leave only virtual ones
    }

    pub fn is_currently_connected(&self) -> bool {
        self.remote.is_parameters_dirty().is_ok()
    }

    pub async fn wait_for_connection(&self) {
        if !self.is_currently_connected() {
            print!(
                "\nCouldn't connect to Voicemeeter. Will retry every 5s.\
                Make sure it is running."
            );

            loop {
                smol::Timer::after(std::time::Duration::from_secs(5)).await;

                if self.is_currently_connected() {
                    break;
                }

                print!(".");
            }

            println!("Connected.");
        }
    }
}
