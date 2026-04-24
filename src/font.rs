//! 字体管理模块
//!
//! 负责系统字体解析、加载和 egui 字体配置

use std::sync::Arc;

const FALLBACK_CACHE_KEY: &str = "__fallback__";

pub struct PreparedFonts {
    definitions: egui::FontDefinitions,
    config_changed: bool,
}

pub struct LoadedFont {
    pub id: String,
    pub data: Arc<egui::FontData>,
    pub cache_path: Option<String>,
}

#[derive(Default)]
pub struct FontResolver {
    db: Option<fontdb::Database>,
}

impl FontResolver {
    pub fn resolve(
        &mut self,
        configured_name: Option<&str>,
        configured_path: Option<&str>,
        cached_name: Option<&str>,
        cached_path: Option<&str>,
        fallback_names: &[&str],
    ) -> Option<LoadedFont> {
        if let Some(font) = Self::load_from_path(configured_path) {
            return Some(font);
        }

        let cache_key = cache_key(configured_name);
        if cached_name == Some(cache_key) {
            if let Some(font) = Self::load_from_path(cached_path) {
                return Some(font);
            }
        }

        if let Some(name) = configured_name {
            if let Some(font) = self.find_by_name(name) {
                return Some(font);
            }
            tracing::warn!("Configured font '{name}' was not found in system fonts");
        }

        for fallback in fallback_names {
            if let Some(font) = self.find_by_name(fallback) {
                return Some(font);
            }
        }

        None
    }

    fn load_from_path(configured_path: Option<&str>) -> Option<LoadedFont> {
        let path = std::path::Path::new(configured_path?);
        if !path.is_file() {
            tracing::warn!("Configured font path does not exist: {}", path.display());
            return None;
        }

        let data = match std::fs::read(path) {
            Ok(data) => data,
            Err(err) => {
                tracing::warn!("Failed to read font file {}: {}", path.display(), err);
                return None;
            }
        };

        Some(LoadedFont {
            id: format!("font-file:{}", path.to_string_lossy()),
            data: Arc::new(egui::FontData::from_owned(data)),
            cache_path: Some(path.to_string_lossy().to_string()),
        })
    }

    fn find_by_name(&mut self, name: &str) -> Option<LoadedFont> {
        let db = self.system_db();
        let query = fontdb::Query {
            families: &[fontdb::Family::Name(name)],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };
        let id = db.query(&query)?;
        let face = db.face(id)?;
        let (data, cache_path) = match &face.source {
            fontdb::Source::Binary(bin) => (bin.as_ref().as_ref().to_vec(), None),
            fontdb::Source::File(path) => (
                std::fs::read(path).ok()?,
                Some(path.to_string_lossy().to_string()),
            ),
            fontdb::Source::SharedFile(path, _) => (
                std::fs::read(path).ok()?,
                Some(path.to_string_lossy().to_string()),
            ),
        };
        let family_name = face.families.first()?.0.clone();

        Some(LoadedFont {
            id: format!("font-family:{family_name}"),
            data: Arc::new(egui::FontData::from_owned(data)),
            cache_path,
        })
    }

    fn system_db(&mut self) -> &mut fontdb::Database {
        self.db.get_or_insert_with(|| {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            db
        })
    }
}

fn cache_key(configured_name: Option<&str>) -> &str {
    configured_name.unwrap_or(FALLBACK_CACHE_KEY)
}

fn proportional_fallbacks() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        &["Microsoft YaHei", "Segoe UI", "Arial"]
    }
    #[cfg(target_os = "macos")]
    {
        &["PingFang SC", "SF Pro Text", "Helvetica Neue"]
    }
    #[cfg(target_os = "linux")]
    {
        &[
            "Noto Sans CJK SC",
            "Noto Sans",
            "DejaVu Sans",
            "Liberation Sans",
        ]
    }
}

fn monospace_fallbacks() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        &["Cascadia Mono", "Consolas", "Courier New"]
    }
    #[cfg(target_os = "macos")]
    {
        &["SF Mono", "Menlo", "Monaco"]
    }
    #[cfg(target_os = "linux")]
    {
        &[
            "JetBrains Mono",
            "DejaVu Sans Mono",
            "Liberation Mono",
            "Monospace",
        ]
    }
}

fn install_font_override(
    fonts: &mut egui::FontDefinitions,
    family: egui::FontFamily,
    resolver: &mut FontResolver,
    configured_name: Option<&str>,
    configured_path: Option<&str>,
    cached_name: Option<&str>,
    cached_path: Option<&str>,
    fallback_names: &[&str],
) -> (Option<String>, Option<String>, Option<String>) {
    if let Some(font) = resolver.resolve(
        configured_name,
        configured_path,
        cached_name,
        cached_path,
        fallback_names,
    ) {
        let font_id = font.id.clone();
        let cache_name = if configured_path.is_none() && font.cache_path.is_some() {
            Some(cache_key(configured_name).to_string())
        } else {
            None
        };
        let cache_path = if cache_name.is_some() {
            font.cache_path.clone()
        } else {
            None
        };
        let family_fonts = fonts.families.entry(family).or_default();
        if !family_fonts.iter().any(|existing| existing == &font_id) {
            family_fonts.insert(0, font_id.clone());
        }
        fonts.font_data.insert(font.id, font.data);
        (Some(font_id), cache_name, cache_path)
    } else {
        (None, None, None)
    }
}

fn append_family_fallback(
    fonts: &mut egui::FontDefinitions,
    family: egui::FontFamily,
    fallback_font_id: Option<&str>,
) {
    let Some(fallback_font_id) = fallback_font_id else {
        return;
    };

    let family_fonts = fonts.families.entry(family).or_default();
    if !family_fonts
        .iter()
        .any(|existing| existing == fallback_font_id)
    {
        family_fonts.push(fallback_font_id.to_owned());
    }
}

impl PreparedFonts {
    pub fn config_changed(&self) -> bool {
        self.config_changed
    }
}

/// 预先解析字体配置，避免在窗口创建阶段做系统字体探测和磁盘读取
pub fn prepare_fonts(config: &mut crate::config::AppConfig) -> PreparedFonts {
    let mut fonts = egui::FontDefinitions::default();
    let mut resolver = FontResolver::default();
    let mut config_changed = false;

    let (proportional_font, ui_cached_name, ui_cached_path) = install_font_override(
        &mut fonts,
        egui::FontFamily::Proportional,
        &mut resolver,
        config.ui_font_name.as_deref(),
        config.ui_font_path.as_deref(),
        config.ui_font_cached_name.as_deref(),
        config.ui_font_cached_path.as_deref(),
        proportional_fallbacks(),
    );
    let (_monospace_font, code_cached_name, code_cached_path) = install_font_override(
        &mut fonts,
        egui::FontFamily::Monospace,
        &mut resolver,
        config.code_font_name.as_deref(),
        config.code_font_path.as_deref(),
        config.code_font_cached_name.as_deref(),
        config.code_font_cached_path.as_deref(),
        monospace_fallbacks(),
    );

    if config.ui_font_cached_name != ui_cached_name {
        config.ui_font_cached_name = ui_cached_name;
        config_changed = true;
    }
    if config.ui_font_cached_path != ui_cached_path {
        config.ui_font_cached_path = ui_cached_path;
        config_changed = true;
    }
    if config.code_font_cached_name != code_cached_name {
        config.code_font_cached_name = code_cached_name;
        config_changed = true;
    }
    if config.code_font_cached_path != code_cached_path {
        config.code_font_cached_path = code_cached_path;
        config_changed = true;
    }

    // Let code blocks keep their monospace primary font, but fall back to the UI
    // font family for CJK glyphs that many monospace fonts do not provide.
    append_family_fallback(
        &mut fonts,
        egui::FontFamily::Monospace,
        proportional_font.as_deref(),
    );

    PreparedFonts {
        definitions: fonts,
        config_changed,
    }
}

/// 将预先准备好的字体定义应用到 egui
pub fn apply_prepared_fonts(ctx: &egui::Context, prepared: PreparedFonts) {
    ctx.set_fonts(prepared.definitions);
}
