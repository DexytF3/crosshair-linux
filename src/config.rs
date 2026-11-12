use crate::model::Config;
use std::fs;
use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    glib::user_config_dir().join("crosshair-linux")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn data_dir() -> PathBuf {
    glib::user_data_dir().join("crosshair-linux")
}

pub fn custom_crosshair_dir() -> PathBuf {
    data_dir().join("crosshairs")
}

pub fn ensure_dirs() -> std::io::Result<()> {
    fs::create_dir_all(config_dir())?;
    fs::create_dir_all(custom_crosshair_dir())?;
    Ok(())
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        match fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<Config>(&text) {
                Ok(cfg) => cfg,
                Err(err) => {
                    eprintln!("crosshair-linux: could not parse {}: {err}", path.display());
                    Config::default()
                }
            },
            Err(_) => Config::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        ensure_dirs().map_err(|e| format!("create config dirs: {e}"))?;
        let path = config_path();
        let text = toml::to_string_pretty(self).map_err(|e| format!("serialize config: {e}"))?;
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        fs::rename(&tmp, &path).map_err(|e| format!("rename config: {e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip() {
        let mut cfg = Config::default();
        cfg.style.size = 123;
        cfg.position.mode = crate::model::PositionMode::Pixel;
        cfg.position.x = 42;
        cfg.theme = crate::model::Theme::Amoled;
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn defaults_on_empty_document() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg.style.size, 48);
        assert_eq!(cfg.style.crosshair, crate::model::DEFAULT_CROSSHAIR);
    }
}
