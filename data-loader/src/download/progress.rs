//! Enhanced download progress tracking and management structures
//!
//! This module provides detailed types for tracking download progress and status
//! for historical data downloads (klines, trades, etc.). Adapted from flowsurface.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Detailed progress information for an active download
#[derive(Debug, Clone)]
pub struct DetailedDownloadProgress {
    // File-level progress
    pub total_files: usize,
    pub completed_files: usize,
    pub failed_files: usize,
    pub current_file: Option<String>,

    // Byte-level progress
    pub bytes_downloaded: u64,
    pub total_bytes_estimate: u64,

    // Record-level progress
    pub records_inserted: u64,
    pub records_failed: u64,

    // Performance metrics
    pub speed_mbps: f64,
    pub speed_records_per_sec: f64,
    pub eta_seconds: Option<u64>,

    // Status
    pub status: DownloadStatus,
    pub started_at: Instant,
}

impl DetailedDownloadProgress {
    pub fn new(total_files: usize) -> Self {
        Self {
            total_files,
            completed_files: 0,
            failed_files: 0,
            current_file: None,
            bytes_downloaded: 0,
            total_bytes_estimate: 0,
            records_inserted: 0,
            records_failed: 0,
            speed_mbps: 0.0,
            speed_records_per_sec: 0.0,
            eta_seconds: None,
            status: DownloadStatus::Initializing,
            started_at: Instant::now(),
        }
    }

    /// Calculate current progress percentage (0-100)
    pub fn progress_percent(&self) -> f32 {
        if self.total_files == 0 {
            return 0.0;
        }
        (self.completed_files as f32 / self.total_files as f32) * 100.0
    }

    /// Update performance metrics based on elapsed time
    pub fn update_metrics(&mut self) {
        let elapsed = self.started_at.elapsed();
        let elapsed_secs = elapsed.as_secs_f64();

        if elapsed_secs > 0.0 {
            // Calculate speed in MB/s
            self.speed_mbps = (self.bytes_downloaded as f64 / 1_048_576.0) / elapsed_secs;

            // Calculate records per second
            self.speed_records_per_sec = self.records_inserted as f64 / elapsed_secs;

            // Estimate remaining time
            if self.completed_files > 0 {
                let avg_time_per_file = elapsed_secs / self.completed_files as f64;
                let remaining_files = self.total_files.saturating_sub(self.completed_files);
                self.eta_seconds = Some((remaining_files as f64 * avg_time_per_file) as u64);
            }
        }
    }
}

/// Current status of a download
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadStatus {
    Initializing,
    Downloading,
    Paused,
    Validating,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for DownloadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadStatus::Initializing => write!(f, "Initializing"),
            DownloadStatus::Downloading => write!(f, "Downloading"),
            DownloadStatus::Paused => write!(f, "Paused"),
            DownloadStatus::Validating => write!(f, "Validating"),
            DownloadStatus::Completed => write!(f, "Completed"),
            DownloadStatus::Failed => write!(f, "Failed"),
            DownloadStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Summary statistics after download completes
#[derive(Debug, Clone)]
pub struct DownloadStats {
    pub total_files: usize,
    pub successful_files: usize,
    pub failed_files: usize,
    pub total_records: u64,
    pub total_bytes: u64,
    pub duration: Duration,
    pub errors: Vec<DownloadError>,
}

impl DownloadStats {
    pub fn success_rate(&self) -> f32 {
        if self.total_files == 0 {
            return 0.0;
        }
        (self.successful_files as f32 / self.total_files as f32) * 100.0
    }

    pub fn average_speed_mbps(&self) -> f64 {
        let secs = self.duration.as_secs_f64();
        if secs > 0.0 {
            (self.total_bytes as f64 / 1_048_576.0) / secs
        } else {
            0.0
        }
    }
}

/// Error that occurred during download
#[derive(Debug, Clone)]
pub struct DownloadError {
    pub file_name: String,
    pub date: chrono::NaiveDate,
    pub error: String,
    pub retry_count: u32,
}

/// Configuration for a kline download job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineDownloadConfig {
    pub ticker: String,
    pub exchange: String,
    pub interval: String,
    pub start_date: chrono::NaiveDate,
    pub end_date: chrono::NaiveDate,
    pub skip_existing: bool,
}

impl KlineDownloadConfig {
    /// Generate list of dates to download
    pub fn generate_date_range(&self) -> Vec<chrono::NaiveDate> {
        let mut dates = Vec::new();
        let mut current = self.start_date;

        while current <= self.end_date {
            dates.push(current);
            current = current.succ_opt().expect("Date overflow");
        }

        dates
    }

    /// Estimate total files to download
    pub fn estimate_file_count(&self) -> usize {
        self.generate_date_range().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_percent() {
        let mut progress = DetailedDownloadProgress::new(100);
        assert_eq!(progress.progress_percent(), 0.0);

        progress.completed_files = 50;
        assert_eq!(progress.progress_percent(), 50.0);

        progress.completed_files = 100;
        assert_eq!(progress.progress_percent(), 100.0);
    }

    #[test]
    fn test_download_config_date_range() {
        let config = KlineDownloadConfig {
            ticker: "BTCUSDT".to_string(),
            exchange: "BinanceLinear".to_string(),
            interval: "1h".to_string(),
            start_date: chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            end_date: chrono::NaiveDate::from_ymd_opt(2024, 1, 5).unwrap(),
            skip_existing: false,
        };

        let dates = config.generate_date_range();
        assert_eq!(dates.len(), 5);
        assert_eq!(dates[0], chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
        assert_eq!(dates[4], chrono::NaiveDate::from_ymd_opt(2024, 1, 5).unwrap());
    }

    #[test]
    fn test_stats_success_rate() {
        let stats = DownloadStats {
            total_files: 100,
            successful_files: 95,
            failed_files: 5,
            total_records: 10000,
            total_bytes: 1000000,
            duration: Duration::from_secs(10),
            errors: vec![],
        };

        assert_eq!(stats.success_rate(), 95.0);
    }
}
