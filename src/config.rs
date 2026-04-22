use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub window_width: f32,
    pub window_height: f32,
    pub window_x: Option<f32>,
    pub window_y: Option<f32>,
    pub maximized: bool,
    pub theme_name: Option<String>,
    pub font_size: f32,
    #[serde(default, alias = "font_name")]
    pub ui_font_name: Option<String>,
    #[serde(default, alias = "font_path")]
    pub ui_font_path: Option<String>,
    #[serde(default)]
    pub code_font_name: Option<String>,
    #[serde(default)]
    pub code_font_path: Option<String>,
    pub last_file: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            window_width: 900.0,
            window_height: 700.0,
            window_x: None,
            window_y: None,
            maximized: false,
            theme_name: None,
            font_size: 16.0,
            ui_font_name: None,
            ui_font_path: None,
            code_font_name: None,
            code_font_path: None,
            last_file: None,
        }
    }
}

impl AppConfig {
    fn config_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("mdview");
        std::fs::create_dir_all(&config_dir).ok();
        config_dir.join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            toml::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        let content = toml::to_string_pretty(self).map_err(|e| format!("序列化配置失败: {}", e))?;
        std::fs::write(&path, content).map_err(|e| format!("保存配置失败: {}", e))
    }
}
