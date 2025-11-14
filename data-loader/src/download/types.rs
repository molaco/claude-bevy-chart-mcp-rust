//! Type definitions for download operations

use std::path::PathBuf;

/// Progress information during download operations
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub current_file: String,
    pub current_index: usize,
    pub total_files: usize,
    pub bytes_downloaded: u64,
}

/// Result of a download operation
#[derive(Debug)]
pub struct DownloadResult {
    pub files_downloaded: usize,
    pub files_skipped: usize,
    pub total_bytes: u64,
    pub downloaded_files: Vec<PathBuf>,
    pub errors: Vec<String>,
}

impl DownloadResult {
    pub fn new() -> Self {
        Self {
            files_downloaded: 0,
            files_skipped: 0,
            total_bytes: 0,
            downloaded_files: Vec::new(),
            errors: Vec::new(),
        }
    }
}

impl Default for DownloadResult {
    fn default() -> Self {
        Self::new()
    }
}
