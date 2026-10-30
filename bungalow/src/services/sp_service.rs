//! Port of `SPService`: the base of all Bungalow services (media, streaming, social, …).

pub trait SPService {
    /// True if this service can handle the given URI scheme/host.
    fn accepts_uri(&self, uri: &str) -> bool;

    /// Initiates the login flow (opens browser, shows dialog, etc.).
    fn start_login(&self);

    /// Logs the current user out and clears any stored credentials.
    fn logout(&self);

    /// Stable machine-readable identifier (e.g. "local", "spotify").
    fn get_id(&self) -> String;

    /// Human-readable display name (e.g. "Local Media", "Spotify").
    fn get_name(&self) -> String;

    /// True when the service has an active authenticated session.
    fn is_logged_in(&self) -> bool;

    /// The display name of the logged-in user, or "" when not logged in.
    fn get_logged_in_user(&self) -> String;
}
