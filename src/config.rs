use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub browser: BrowserConfig,
}

#[derive(Debug, Deserialize)]
pub struct BrowserConfig {
    pub executable: String,
    pub port: u16,
    #[serde(default)]
    pub restore_session: bool,
    #[serde(default)]
    pub dedicated_profile: bool,
}

pub fn load_config(path: &str) -> Result<Config, ConfigError> {
    let contents =
        std::fs::read_to_string(path).map_err(|_| ConfigError::FileNotFound(path.to_owned()))?;
    let config: Config =
        toml::from_str(&contents).map_err(|e| ConfigError::ParseError(e.to_string()))?;
    Ok(config)
}

#[derive(Debug)]
pub enum ConfigError {
    FileNotFound(String),
    ParseError(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::FileNotFound(path) => write!(f, "Config file not found: {path}"),
            ConfigError::ParseError(msg) => write!(f, "Config parse error: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {}
