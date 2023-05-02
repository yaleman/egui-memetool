//! Config things

use anyhow::Context;
use log::*;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

const CONFIG_PATH: &str = "~/.config/memetool.json";

#[derive(Clone, Deserialize, Serialize)]
pub struct Configuration {
    pub s3_access_key_id: String,
    pub s3_secret_access_key: String,
    pub s3_bucket: String,
    pub s3_region: String,
    // Set a custom endpoint, for example if you're using minio or another alternate S3 provider
    pub s3_endpoint: Option<String>,
}

impl Configuration {
    pub fn try_new() -> anyhow::Result<Self> {
        let shellpath = shellexpand::tilde(CONFIG_PATH);
        let configpath = std::path::PathBuf::from(shellpath.as_ref());
        let mut confighandle = std::fs::File::open(configpath)
            .with_context(|| format!("Failed to open configuration file {}", CONFIG_PATH))?;
        let mut configcontents = String::new();

        #[allow(clippy::unwrap_used)]
        confighandle.read_to_string(&mut configcontents)?;

        serde_json::from_str(&configcontents)
            .with_context(|| format!("Failed to parse configuration file {}", CONFIG_PATH))
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let shellpath = shellexpand::tilde(CONFIG_PATH);
        let configpath = std::path::PathBuf::from(shellpath.as_ref());
        let configcontents = serde_json::to_string_pretty(self)?;
        let mut confighandle = std::fs::File::create(configpath)
            .with_context(|| format!("Failed to open configuration file {}", CONFIG_PATH))?;
        // write the config file to confighandle
        confighandle
            .write(configcontents.as_bytes())
            .with_context(|| format!("Failed to write configuration file {}", CONFIG_PATH))?;
        info!("Successfully wrote config to {}", CONFIG_PATH);
        Ok(())
    }
}
