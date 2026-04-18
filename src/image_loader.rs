//! Asynchronous image loader
//!
//! Supports:
//! - Local files (relative to .md document directory, absolute paths, file:// URIs)
//! - HTTP/HTTPS URLs (async download)
//! - data: URIs (base64 embedded)

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

use egui::{Context, TextureHandle, TextureOptions};

use base64::Engine;

/// Messages sent from loader threads
enum ImageMsg {
    Ready {
        key: String,
        texture: TextureHandle,
    },
    Failed {
        key: String,
    },
}

/// State of an image in the cache
pub enum ImageState {
    Loading,
    Ready(TextureHandle),
    Failed,
}

/// Async image loader with caching
pub struct ImageLoader {
    /// Base directory for resolving relative paths (the .md file's directory)
    base_dir: PathBuf,
    /// Image cache: key = resolved URL/path
    cache: HashMap<String, ImageState>,
    /// Channel for receiving loaded images
    rx: Receiver<ImageMsg>,
    /// Channel for sending load requests (cloned to worker threads)
    tx: Sender<ImageMsg>,
    /// egui context for creating textures on the main thread
    ctx: Option<Context>,
}

impl ImageLoader {
    pub fn new(base_dir: PathBuf) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            base_dir,
            cache: HashMap::new(),
            rx,
            tx,
            ctx: None,
        }
    }

    /// Set the egui context (called once after app creation)
    pub fn set_context(&mut self, ctx: Context) {
        self.ctx = Some(ctx);
    }

    /// Update the base directory (when opening a new file)
    pub fn set_base_dir(&mut self, dir: PathBuf) {
        if dir != self.base_dir {
            self.base_dir = dir;
            self.cache.clear();
        }
    }

    /// Poll for completed image loads. Must be called every frame.
    /// Returns true if any image finished loading (caller should request repaint if needed).
    pub fn poll(&mut self) -> bool {
        let mut any_ready = false;
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                ImageMsg::Ready { key, texture } => {
                    self.cache.insert(key, ImageState::Ready(texture));
                    any_ready = true;
                }
                ImageMsg::Failed { key } => {
                    self.cache.insert(key, ImageState::Failed);
                    any_ready = true;
                }
            }
        }
        any_ready
    }

    /// Get the image state, or start loading if not cached.
    pub fn get(&mut self, url: &str) -> &ImageState {
        // Check cache with original key
        if self.cache.contains_key(url) {
            return self.cache.get(url).unwrap();
        }

        // Resolve the actual path
        let resolved = self.resolve_path(url);

        // If resolved differs from original, check cache under resolved key
        if resolved != url {
            if self.cache.contains_key(&resolved) {
                // Already loading/loaded under resolved key — just alias the original key
                let state = match &self.cache[&resolved] {
                    ImageState::Ready(_) => "ready",
                    ImageState::Failed => "failed",
                    ImageState::Loading => "loading",
                };
                let alias = match state {
                    "ready" => {
                        // Can't clone TextureHandle easily, return Loading for original key
                        // The resolved key will be used for actual display
                        ImageState::Loading
                    }
                    "failed" => ImageState::Failed,
                    _ => ImageState::Loading,
                };
                self.cache.insert(url.to_string(), alias);
                return self.cache.get(url).unwrap();
            }
        }

        // Not cached anywhere — mark as loading and start async load
        self.cache.insert(url.to_string(), ImageState::Loading);
        if resolved != url {
            self.cache.insert(resolved.clone(), ImageState::Loading);
        }

        // Start async load
        self.start_load(url.to_string(), resolved);

        self.cache.get(url).unwrap()
    }

    /// Resolve a URL to a local path or keep as-is for HTTP/data URIs
    fn resolve_path(&self, url: &str) -> String {
        // data: URI
        if url.starts_with("data:") {
            return url.to_string();
        }

        // HTTP/HTTPS
        if url.starts_with("http://") || url.starts_with("https://") {
            return url.to_string();
        }

        // file:// URI — strip prefix and decode
        if let Some(stripped) = url.strip_prefix("file://") {
            let path = urldecode(stripped);
            if std::path::Path::new(&path).is_absolute() {
                return path;
            }
            // Relative file:// URI — resolve against base_dir
            let resolved = self.base_dir.join(&path);
            return resolved.to_string_lossy().to_string();
        }

        // Absolute local path
        if std::path::Path::new(url).is_absolute() {
            return url.to_string();
        }

        // Relative path — resolve against base_dir
        let resolved = self.base_dir.join(url);
        resolved.to_string_lossy().to_string()
    }

    /// Start loading an image asynchronously
    fn start_load(&self, key: String, resolved: String) {
        let tx = self.tx.clone();
        let ctx = match &self.ctx {
            Some(c) => c.clone(),
            None => return, // No context yet, can't load
        };

        std::thread::spawn(move || {
            match load_image_data(&resolved) {
                Ok(image_data) => {
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [image_data.width as usize, image_data.height as usize],
                        &image_data.rgba,
                    );
                    let texture = ctx.load_texture(&key, color_image, TextureOptions::LINEAR);
                    let _ = tx.send(ImageMsg::Ready { key, texture });
                }
                Err(_) => {
                    let _ = tx.send(ImageMsg::Failed { key });
                }
            }
        });
    }
}

/// Result of image loading
struct ImageData {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

/// Load image data from a URL or file path
fn load_image_data(url: &str) -> Result<ImageData, String> {
    // data: URI
    if let Some(rest) = url.strip_prefix("data:") {
        return load_data_uri(rest);
    }

    // HTTP/HTTPS
    if url.starts_with("http://") || url.starts_with("https://") {
        return load_http_image(url);
    }

    // Local file
    load_local_image(url)
}

/// Load a local image file
fn load_local_image(path: &str) -> Result<ImageData, String> {
    let data = std::fs::read(path).map_err(|e| format!("Failed to read {}: {}", path, e))?;
    decode_image(&data)
}

/// Load an image from HTTP/HTTPS
fn load_http_image(url: &str) -> Result<ImageData, String> {
    let response = reqwest::blocking::Client::new()
        .get(url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let data = response.bytes().map_err(|e| format!("Failed to read response: {}", e))?;
    decode_image(&data)
}

/// Decode a data: URI
fn load_data_uri(rest: &str) -> Result<ImageData, String> {
    // Format: data:[<mediatype>][;base64],<data>
    // Find the comma separating metadata from data
    let comma_pos = rest.find(',').ok_or("Invalid data URI: no comma")?;
    let meta = &rest[..comma_pos];
    let data_part = &rest[comma_pos + 1..];

    let is_base64 = meta.contains(";base64") || meta.contains(";base64,");

    let bytes = if is_base64 {
        base64::engine::general_purpose::STANDARD
            .decode(data_part)
            .map_err(|e| format!("Base64 decode failed: {}", e))?
    } else {
        data_part.as_bytes().to_vec()
    };

    decode_image(&bytes)
}

/// Decode image bytes using the `image` crate
fn decode_image(data: &[u8]) -> Result<ImageData, String> {
    let img = image::load_from_memory(data).map_err(|e| format!("Image decode failed: {}", e))?;
    let rgba = img.to_rgba8();
    Ok(ImageData {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

/// URL percent-decoding for file:// URIs (supports non-ASCII paths)
fn urldecode(s: &str) -> String {
    let mut bytes = Vec::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(h1), Some(h2)) = (h1, h2) {
                let hex = &[h1, h2];
                if let Ok(byte) = u8::from_str_radix(std::str::from_utf8(hex).unwrap_or("0"), 16) {
                    bytes.push(byte);
                } else {
                    bytes.extend_from_slice(&[b'%', h1, h2]);
                }
            } else {
                bytes.push(b'%');
                bytes.extend(h1);
                bytes.extend(h2);
            }
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}
