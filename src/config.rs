use once_cell::sync::Lazy;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct CliPaths {
    pub dnsrecon: String,
    pub wsl: String,
    // Otros comandos CLI pueden añadirse aquí en el futuro
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    pub cli_paths: CliPaths,
}

pub static CLI_PATHS: Lazy<CliPaths> = Lazy::new(|| {
    let base_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let config_path = base_path.join("config.toml");
    let contents = fs::read_to_string(config_path).expect("Failed to read config.toml");
    let config: ConfigFile = toml::from_str(&contents).expect("Failed to parse config.toml");
    config.cli_paths
});
