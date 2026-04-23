//! 简单文件监视器
//!
//! 通过轮询文件修改时间检测变化（每 2 秒检查一次）

use std::path::PathBuf;
use std::time::Instant;

pub struct SimpleFileWatcher {
    path: Option<PathBuf>,
    last_modified: Option<std::time::SystemTime>,
    last_checked: Instant,
}

impl SimpleFileWatcher {
    pub fn new(path: Option<PathBuf>) -> Self {
        let last_modified = path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok());
        Self {
            path,
            last_modified,
            last_checked: Instant::now(),
        }
    }

    /// 检查文件是否被修改，返回 true 表示检测到变化
    pub fn check(&mut self) -> bool {
        if self.last_checked.elapsed() < std::time::Duration::from_secs(2) {
            return false;
        }
        self.last_checked = Instant::now();

        if let Some(path) = &self.path {
            if let Ok(meta) = std::fs::metadata(path) {
                if let Ok(modified) = meta.modified() {
                    if Some(modified) != self.last_modified {
                        self.last_modified = Some(modified);
                        return true;
                    }
                }
            }
        }
        false
    }

    /// 更新监视的文件路径
    #[allow(dead_code)]
    pub fn set_path(&mut self, path: Option<PathBuf>) {
        self.path = path;
        self.last_modified = self
            .path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok());
        self.last_checked = Instant::now();
    }
}
