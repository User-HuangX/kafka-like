use std::sync::LazyLock;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub dao: DaoConfig,
}

#[derive(Deserialize)]
pub struct DaoConfig {
    pub index_path: String,
    pub path: String,
}

fn load_config() -> anyhow::Result<Config> {
    let content = std::fs::read_to_string("./Config.toml")?;

    let config: Config = toml::from_str(&content)?;

    Ok(config)
}

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| load_config().unwrap());
