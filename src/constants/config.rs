//! Client-side names: the OneKey home, profiles and environment variables.
pub const DEFAULT_PUBLIC_URL: &str = "http://localhost:8840";
pub const DATA_DIRECTORY_NAME: &str = ".onekey";
pub const CLIENT_CONFIG_FILENAME: &str = "config.toml";
pub const ENV_DATA_DIR: &str = "ONEKEY_DATA_DIR";
pub const ENV_RUN_ENVIRONMENT: &str = "ONEKEY_ENV";
pub const ENV_SERVER_URL: &str = "ONEKEY_URL";
pub const ENV_TOKEN: &str = "ONEKEY_TOKEN";
pub const ENV_PROFILE: &str = "ONEKEY_PROFILE";
/// File in the OneKey home naming the active profile (`onekey profile use`).
pub const ACTIVE_PROFILE_FILENAME: &str = "profile";
/// Directory under the OneKey home holding one client data directory per named profile.
pub const PROFILES_DIRECTORY_NAME: &str = "profiles";
/// The profile that lives directly in the OneKey home (pre-profile layout).
pub const DEFAULT_PROFILE: &str = "default";

/// Every environment variable this CLI reads.
pub const fn executable_environment_names() -> [&'static str; 5] {
  [
    ENV_TOKEN,
    ENV_SERVER_URL,
    ENV_PROFILE,
    ENV_RUN_ENVIRONMENT,
    ENV_DATA_DIR,
  ]
}
