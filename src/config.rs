// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2024 Riccardo Iaconelli <riccardo@kde.org>

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;

use crate::model::Config;

pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new() -> Result<Self> {
        let path = config_path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .with_context(|| format!("failed to create config dir {}", dir.display()))?;
        }
        Ok(Self { path })
    }

    #[cfg(test)]
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load_or_init(&self) -> Result<Config> {
        if self.path.exists() {
            let content =
                fs::read_to_string(&self.path).with_context(|| "failed to read config file")?;
            let cfg: Config = toml::from_str(&content)
                .with_context(|| "failed to parse config; fix or remove the file")?;
            return Ok(cfg);
        }

        let cfg = Config::default();
        self.save(&cfg)?;
        Ok(cfg)
    }

    pub fn save(&self, config: &Config) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)
                .with_context(|| format!("failed to create config dir {}", dir.display()))?;
        }
        if self.path.exists() {
            let backup = self.path.with_extension("toml.bak");
            fs::copy(&self.path, &backup).ok();
        }

        let toml =
            toml::to_string_pretty(config).with_context(|| "failed to serialize config to toml")?;
        let mut f = fs::File::create(&self.path)
            .with_context(|| format!("failed to open config {}", self.path.display()))?;
        f.write_all(toml.as_bytes())
            .with_context(|| "failed to write config")?;
        Ok(())
    }
}

fn config_path() -> PathBuf {
    if let Some(proj) = ProjectDirs::from("", "", "sshdb") {
        return proj.config_dir().join("config.toml");
    }
    dirs_fallback()
}

fn dirs_fallback() -> PathBuf {
    env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".sshdb")
        .join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn saves_and_loads_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let store = ConfigStore { path };
        let cfg = Config::sample();
        store.save(&cfg).unwrap();
        let loaded = store.load_or_init().unwrap();
        assert_eq!(loaded.hosts.len(), cfg.hosts.len());
        assert_eq!(loaded.version, cfg.version);
    }
}
