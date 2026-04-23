//! 字体管理模块
//!
//! 负责系统字体解析、加载和 egui 字体配置

use std::sync::Arc;

pub struct LoadedFont {
    pub id: String,
    pub data: Arc<egui::FontData>,
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
        fallback_names: &[&str],
    ) -> Option<LoadedFont> {
        if let Some(font) = Self::load_from_path(configured_path) {
            return Some(font);
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
        let data = match &face.source {
            fontdb::Source::Binary(bin) => bin.as_ref().as_ref().to_vec(),
            fontdb::Source::File(path) => std::fs::read(path).ok()?,
            fontdb::Source::SharedFile(path, _) => std::fs::read(path).ok()?,
        };
        let family_name = face.families.first()?.0.clone();

        Some(LoadedFont {
            id: format!("font-family:{family_name}"),
            data: Arc::new(egui::FontData::from_owned(data)),
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
    fallback_names: &[&str],
) {
    if let Some(font) = resolver.resolve(configured_name, configured_path, fallback_names) {
        let family_fonts = fonts.families.entry(family).or_default();
        if !family_fonts.iter().any(|existing| existing == &font.id) {
            family_fonts.insert(0, font.id.clone());
        }
        fonts.font_data.insert(font.id, font.data);
    }
}

/// 根据配置设置 egui 字体
pub fn setup_fonts(ctx: &egui::Context, config: &crate::config::AppConfig) {
    let mut fonts = egui::FontDefinitions::default();
    let mut resolver = FontResolver::default();

    install_font_override(
        &mut fonts,
        egui::FontFamily::Proportional,
        &mut resolver,
        config.ui_font_name.as_deref(),
        config.ui_font_path.as_deref(),
        proportional_fallbacks(),
    );
    install_font_override(
        &mut fonts,
        egui::FontFamily::Monospace,
        &mut resolver,
        config.code_font_name.as_deref(),
        config.code_font_path.as_deref(),
        monospace_fallbacks(),
    );

    ctx.set_fonts(fonts);
}
