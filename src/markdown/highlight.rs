//! Syntax highlighting using syntect → egui LayoutJob

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle as SyntectFontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use egui::text::{LayoutJob, LayoutSection, TextFormat, TextWrapping};
use egui::{Color32, FontFamily, FontId};

struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl Highlighter {
    fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_nonewlines();
        let theme_set = ThemeSet::load_defaults();
        Self {
            syntax_set,
            theme_set,
        }
    }

    fn highlight(
        &self,
        code: &str,
        lang: &str,
        theme_name: &str,
        font_size: f32,
    ) -> Option<LayoutJob> {
        let syntax = self
            .syntax_set
            .find_syntax_by_token(lang)
            .or_else(|| self.syntax_set.find_syntax_by_extension(lang))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = match self.theme_set.themes.get(theme_name) {
            Some(t) => t,
            None => self.theme_set.themes.get("base16-ocean.dark")?,
        };

        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut job = LayoutJob {
            text: String::new(),
            wrap: TextWrapping::no_max_width(),
            ..Default::default()
        };

        for line in LinesWithEndings::from(code) {
            let ranges = highlighter.highlight_line(line, &self.syntax_set).ok()?;
            for (style, text) in ranges {
                let fg = style.foreground;
                let color = Color32::from_rgb(fg.r, fg.g, fg.b);
                let clean_text: String = text.replace('\n', "");

                if clean_text.is_empty() {
                    continue;
                }

                let start = job.text.len();
                job.text.push_str(&clean_text);
                let end = job.text.len();

                let is_bold = style.font_style.contains(SyntectFontStyle::BOLD);
                let is_italic = style.font_style.contains(SyntectFontStyle::ITALIC);
                let effective_size = if is_bold { font_size * 1.05 } else { font_size };

                job.sections.push(LayoutSection {
                    leading_space: 0.0,
                    byte_range: start..end,
                    format: TextFormat {
                        font_id: FontId::new(effective_size, FontFamily::Monospace),
                        color,
                        italics: is_italic,
                        ..Default::default()
                    },
                });
            }
            // Add newline character for line breaks
            let start = job.text.len();
            job.text.push('\n');
            let end = job.text.len();
            job.sections.push(LayoutSection {
                leading_space: 0.0,
                byte_range: start..end,
                format: TextFormat {
                    font_id: FontId::new(font_size, FontFamily::Monospace),
                    color: Color32::TRANSPARENT, // invisible newline
                    ..Default::default()
                },
            });
        }

        // Remove trailing newline if present
        if job.text.ends_with('\n') {
            job.text.pop();
            if let Some(last) = job.sections.last_mut() {
                last.byte_range.end = job.text.len();
                if last.byte_range.start >= last.byte_range.end {
                    job.sections.pop();
                }
            }
        }

        Some(job)
    }
}

static HIGHLIGHTER: OnceLock<Highlighter> = OnceLock::new();
static HIGHLIGHT_CACHE: OnceLock<Mutex<HashMap<u64, LayoutJob>>> = OnceLock::new();

fn cache_key(code: &str, lang: &str, theme_name: &str, font_size: f32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut hasher);
    lang.hash(&mut hasher);
    theme_name.hash(&mut hasher);
    font_size.to_bits().hash(&mut hasher);
    hasher.finish()
}

/// Highlight code and return a LayoutJob suitable for egui rendering.
/// Returns None if highlighting fails (caller should fall back to plain text).
/// Results are cached to avoid re-highlighting every frame.
pub fn highlight_code(
    code: &str,
    lang: &str,
    theme_name: &str,
    font_size: f32,
) -> Option<LayoutJob> {
    let key = cache_key(code, lang, theme_name, font_size);
    let cache = HIGHLIGHT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    // Check cache first
    if let Ok(guard) = cache.lock() {
        if let Some(job) = guard.get(&key) {
            return Some(job.clone());
        }
    }

    // Cache miss - do the highlight
    let highlighter = HIGHLIGHTER.get_or_init(Highlighter::new);
    let job = highlighter.highlight(code, lang, theme_name, font_size)?;

    // Store in cache
    if let Ok(mut guard) = cache.lock() {
        // Limit cache size to prevent unbounded growth
        if guard.len() > 512 {
            guard.clear();
        }
        guard.insert(key, job.clone());
    }

    Some(job)
}

/// Clear the highlight cache (call when theme or font size changes)
pub fn clear_highlight_cache() {
    if let Some(cache) = HIGHLIGHT_CACHE.get() {
        if let Ok(mut guard) = cache.lock() {
            guard.clear();
        }
    }
}
