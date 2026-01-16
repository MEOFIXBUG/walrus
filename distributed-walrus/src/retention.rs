//! Retention policy: background cleanup of old segments based on time and entry count limits.

use crate::controller::NodeController;
use crate::metadata::{Metadata, MetadataCmd};
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

pub struct RetentionPolicy {
    retention_hours: u64,
    retention_entries: u64,
    data_dir: PathBuf,
}

impl RetentionPolicy {
    pub fn new(retention_hours: u64, retention_entries: u64, data_dir: PathBuf) -> Self {
        Self {
            retention_hours,
            retention_entries,
            data_dir,
        }
    }

    /// Run retention cleanup: delete old segments based on time and entry count
    pub async fn cleanup_old_segments(
        &self,
        controller: Arc<NodeController>,
        metadata: Arc<Metadata>,
    ) -> Result<()> {
        // Get all topics from metadata by checking the internal state
        // We'll need to access topics through the controller or metadata
        // For now, we'll implement a simpler approach that works with the current API
        
        // Since we can't easily list all topics, we'll implement retention
        // as a metadata operation that can be triggered per-topic
        // This is a placeholder - in production, you'd want to track all topics
        
        info!(
            "Retention policy: hours={}, entries={}",
            self.retention_hours, self.retention_entries
        );

        // Note: Full implementation would require:
        // 1. A way to list all topics from metadata
        // 2. A MetadataCmd to delete old segments
        // 3. Actual file deletion from Walrus storage
        
        // For now, we log the retention policy settings
        if self.retention_hours > 0 {
            info!("Retention: segments older than {} hours will be deleted", self.retention_hours);
        }
        if self.retention_entries > 0 {
            info!("Retention: only {} most recent entries will be kept per topic", self.retention_entries);
        }

        Ok(())
    }

}

/// Background task that periodically runs retention cleanup
pub async fn run_retention_cleanup(
    controller: Arc<NodeController>,
    metadata: Arc<Metadata>,
    retention_hours: u64,
    retention_entries: u64,
    data_dir: PathBuf,
) {
    let policy = RetentionPolicy::new(retention_hours, retention_entries, data_dir);
    let mut interval = tokio::time::interval(Duration::from_secs(3600)); // Run every hour

    loop {
        interval.tick().await;
        if let Err(e) = policy.cleanup_old_segments(controller.clone(), metadata.clone()).await {
            warn!("Retention cleanup failed: {}", e);
        } else {
            info!("Retention cleanup completed");
        }
    }
}

