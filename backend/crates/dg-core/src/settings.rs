//! Settings (dropgrab-app-design.md section 13, keys as used by the UI in
//! `clients/web/js/mock/data.js` `SETTINGS_DEFAULT`). Values are typed, validated on write, and
//! persisted in SQLite.

use serde_json::{json, Map, Value};

use crate::db::Db;

pub const DEFAULT_VISIBILITY: &str = "everyone";

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub device_name: String,
    pub visibility: String,
    pub save_location: String,
    pub ask_before_accepting: bool,
    pub download_folder: String,
    pub downloads_at_once: u32,
    pub wifi_only: bool,
    pub speed_limit: String,
    pub default_quality: String,
    pub audio_format: String,
    pub theme: String,
    pub text_size: String,
    pub private_mode: bool,
    pub hide_lock_details: bool,
    pub segments_per_file: u32,
}

impl Settings {
    pub fn defaults(device_name: &str, download_folder: &str) -> Settings {
        Settings {
            device_name: device_name.to_string(),
            visibility: DEFAULT_VISIBILITY.into(),
            save_location: "Received".into(),
            ask_before_accepting: true,
            download_folder: download_folder.to_string(),
            downloads_at_once: 5,
            wifi_only: true,
            speed_limit: "off".into(),
            default_quality: "balanced".into(),
            audio_format: "M4A".into(),
            theme: "system".into(),
            text_size: "default".into(),
            private_mode: false,
            hide_lock_details: false,
            segments_per_file: 8,
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "deviceName": self.device_name,
            "visibility": self.visibility,
            "saveLocation": self.save_location,
            "askBeforeAccepting": self.ask_before_accepting,
            "downloadFolder": self.download_folder,
            "downloadsAtOnce": self.downloads_at_once,
            "wifiOnly": self.wifi_only,
            "speedLimit": self.speed_limit,
            "defaultQuality": self.default_quality,
            "audioFormat": self.audio_format,
            "theme": self.theme,
            "textSize": self.text_size,
            "privateMode": self.private_mode,
            "hideLockDetails": self.hide_lock_details,
            "segmentsPerFile": self.segments_per_file,
            "version": format!("{} (backend)", env!("CARGO_PKG_VERSION")),
        })
    }

    /// Loads persisted values over the defaults. Unknown keys in storage are ignored.
    pub fn load(db: &Db, defaults: Settings) -> Result<Settings, String> {
        let mut s = defaults;
        for (k, v) in db.settings_all().map_err(|e| e.to_string())? {
            let value: Value = serde_json::from_str(&v).unwrap_or(Value::Null);
            // Bad stored values fall back to the default instead of failing startup.
            let _ = s.apply_one(&k, &value);
        }
        Ok(s)
    }

    /// Applies a patch from `settings.set`. Returns the list of keys that changed.
    pub fn apply(&mut self, patch: &Map<String, Value>) -> Result<Vec<String>, String> {
        // Validate everything before changing anything.
        let mut trial = self.clone();
        let mut changed = Vec::new();
        for (k, v) in patch {
            if k == "version" {
                continue;
            }
            let before = trial.to_json();
            trial.apply_one(k, v)?;
            if trial.to_json().get(k) != before.get(k) {
                changed.push(k.clone());
            }
        }
        *self = trial;
        Ok(changed)
    }

    fn apply_one(&mut self, key: &str, v: &Value) -> Result<(), String> {
        let str_val = || v.as_str().map(str::to_string).ok_or_else(|| format!("{key} must be a string"));
        let bool_val = || v.as_bool().ok_or_else(|| format!("{key} must be a boolean"));
        let u32_val = |min: u32, max: u32| {
            v.as_u64()
                .filter(|n| *n >= min as u64 && *n <= max as u64)
                .map(|n| n as u32)
                .ok_or_else(|| format!("{key} must be an integer from {min} to {max}"))
        };
        match key {
            "deviceName" => {
                let s = str_val()?;
                let t = s.trim();
                if t.is_empty() || t.chars().count() > 64 {
                    return Err("deviceName must be 1 to 64 characters".into());
                }
                self.device_name = t.to_string();
            }
            "visibility" => {
                let s = str_val()?;
                if !["everyone", "trusted", "hidden"].contains(&s.as_str()) {
                    return Err("visibility must be everyone, trusted or hidden".into());
                }
                self.visibility = s;
            }
            "saveLocation" => self.save_location = str_val()?,
            "askBeforeAccepting" => self.ask_before_accepting = bool_val()?,
            "downloadFolder" => {
                let s = str_val()?;
                if s.trim().is_empty() {
                    return Err("downloadFolder must not be empty".into());
                }
                self.download_folder = s;
            }
            "downloadsAtOnce" => self.downloads_at_once = u32_val(1, 16)?,
            "segmentsPerFile" => self.segments_per_file = u32_val(1, 16)?,
            "wifiOnly" => self.wifi_only = bool_val()?,
            "speedLimit" => self.speed_limit = str_val()?,
            "defaultQuality" => {
                let s = str_val()?;
                if !["best", "balanced", "data-saver", "audio"].contains(&s.as_str()) {
                    return Err("defaultQuality must be best, balanced, data-saver or audio".into());
                }
                self.default_quality = s;
            }
            "audioFormat" => self.audio_format = str_val()?,
            "theme" => {
                let s = str_val()?;
                if !["system", "light", "dark"].contains(&s.as_str()) {
                    return Err("theme must be system, light or dark".into());
                }
                self.theme = s;
            }
            "textSize" => {
                let s = str_val()?;
                if !["default", "large", "largest"].contains(&s.as_str()) {
                    return Err("textSize must be default, large or largest".into());
                }
                self.text_size = s;
            }
            "privateMode" => self.private_mode = bool_val()?,
            "hideLockDetails" => self.hide_lock_details = bool_val()?,
            _ => return Err(format!("unknown setting {key}")),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_rejects_bad_values_without_partial_apply() {
        let mut s = Settings::defaults("Kofi", "/tmp/dl");
        let patch: Map<String, Value> =
            serde_json::from_value(json!({"theme": "dark", "visibility": "loud"})).unwrap();
        assert!(s.apply(&patch).is_err());
        assert_eq!(s.theme, "system", "a failed patch must not partially apply");
    }

    #[test]
    fn apply_reports_changed_keys() {
        let mut s = Settings::defaults("Kofi", "/tmp/dl");
        let patch: Map<String, Value> = serde_json::from_value(json!({"theme": "dark"})).unwrap();
        assert_eq!(s.apply(&patch).unwrap(), vec!["theme".to_string()]);
    }
}
