use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct LinkInner {
    pub remote: ::voicemeeter::VoicemeeterRemote,
}

#[derive(Debug, Clone)]
pub struct Link {
    pub inner: Arc<LinkInner>,
}

/// The link between the app and Voicemeeter
impl Link {
    pub fn new() -> Result<Self, LinkCreationError> {
        ::voicemeeter::VoicemeeterRemote::new()
            .map_err(LinkCreationError::RemoteInit)
            .map(|remote| LinkInner { remote }.into())
            .map(|inner| Self { inner })
    }
}

/// Error that can arise when creating ['VoicemeeterLink'].
#[derive(Debug, thiserror::Error)]
pub enum LinkCreationError {
    #[error("could not create VoicemeeterRemote")]
    RemoteInit(#[from] ::voicemeeter::interface::InitializationError),
}

impl Link {
    pub fn strips(&self) -> impl Iterator<Item = ::voicemeeter::interface::parameters::Strip> {
        let parameters = self.inner.remote.parameters();

        (0..).map_while(move |index| parameters.strip(index).ok())
    }

    pub fn virtual_inputs(
        &self,
    ) -> impl Iterator<Item = ::voicemeeter::interface::parameters::Strip> {
        self.strips() // take all existing strips
            .filter(::voicemeeter::interface::parameters::Strip::is_virtual) // leave only virtual ones
    }

    pub fn gain_parameter(
        &self,
        strip: &::voicemeeter::interface::parameters::Strip,
    ) -> FloatParameter {
        FloatParameter {
            name: strip.param("Gain").into(),
            link: self.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FloatParameter {
    pub name: ::voicemeeter::types::ParameterName,
    pub link: Link,
}

impl FloatParameter {
    pub fn set(
        &self,
        value: f32,
    ) -> Result<(), ::voicemeeter::interface::parameters::set_parameters::SetParameterError> {
        self.link
            .inner
            .remote
            .set_parameter_float(&self.name, value)
    }
}
