#[derive(Debug, Clone)]
pub enum AppAction {
    Toggle,
    Show,
    Hide,
    OpenSettings,
    Quit,
    /// Informational text for the settings window (for example a portal
    /// registration failure).
    Notice(String),
}
