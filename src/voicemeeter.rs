use std::sync::Arc;

#[derive(Debug)]
pub struct LinkInner {
    pub remote: smol::lock::Mutex<::voicemeeter::VoicemeeterRemote>,
}

#[derive(Debug, Clone)]
pub struct Link {
    pub inner: Arc<LinkInner>,
}

/// The link between the app and Voicemeeter, at least one copy of the first one you call must remain alive otherwise things break!
impl Link {
    pub fn new() -> Result<Self, LinkCreationError> {
        ::voicemeeter::VoicemeeterRemote::new()
            .map_err(LinkCreationError::RemoteInit)
            .map(|remote| {
                LinkInner {
                    remote: remote.into(),
                }
                .into()
            })
            .map(|inner| Self { inner })
    }
}

/// Error that can arise when creating [`VoicemeeterLink`].
#[derive(Debug, thiserror::Error)]
pub enum LinkCreationError {
    #[error("{0}")]
    RemoteInit(#[from] ::voicemeeter::interface::InitializationError),
}

impl Link {
    pub fn strips(
        remote: &::voicemeeter::VoicemeeterRemote,
    ) -> impl Iterator<Item = ::voicemeeter::interface::parameters::Strip> {
        let parameters = remote.parameters();

        (0..).map_while(move |index| parameters.strip(index).ok())
    }

    pub fn virtual_inputs(
        remote: &::voicemeeter::VoicemeeterRemote,
    ) -> impl Iterator<Item = ::voicemeeter::interface::parameters::Strip> {
        Self::strips(remote) // take all existing strips
            .filter(::voicemeeter::interface::parameters::Strip::is_virtual) // leave only virtual ones
    }

    pub fn gain_parameter_of(
        &self,
        strip: &::voicemeeter::interface::parameters::Strip,
    ) -> FloatParameter {
        FloatParameter {
            name: strip.param("Gain").into(),
            link: self.clone(),
        }
    }

    pub async fn is_currently_connected(&self) -> bool {
        self.inner.remote.lock().await.is_parameters_dirty().is_ok()
    }

    pub async fn wait_for_connection(&self) {
        if !self.is_currently_connected().await {
            println!(
                "Couldn't connect to Voicemeeter. Will retry every 5s.\
                Make sure it is running."
            );

            loop {
                smol::Timer::after(std::time::Duration::from_secs(5)).await;

                if self.is_currently_connected().await {
                    println!("Connected.");
                    break;
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct FloatParameter {
    pub name: ::voicemeeter::types::ParameterName,
    pub link: Link,
}

impl FloatParameter {
    pub async fn set(
        &self,
        value: f32,
    ) -> Result<(), ::voicemeeter::interface::parameters::set_parameters::SetParameterError> {
        self.link
            .inner
            .remote
            .lock()
            .await
            .set_parameter_float(&self.name, value)
    }
}
