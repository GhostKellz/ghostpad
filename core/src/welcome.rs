//! Welcome screen context data shared with the UI layer.

/// Initial data used by the welcome tab when the application launches.
#[derive(Clone, Debug)]
pub struct WelcomeContext {
    pub headline: &'static str,
    pub tagline: &'static str,
}

pub fn default_welcome_context() -> WelcomeContext {
    WelcomeContext {
        headline: "Let’s get writing",
        tagline: "GhostPad brings Windows 11 calm to Plasma’s power.",
    }
}
