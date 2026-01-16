use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// Retention policy for a topic
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Maximum age of segments in hours (None = no time limit)
    pub max_age_hours: Option<u64>,

    /// Maximum number of segments to keep (None = no segment limit)
    pub max_segments: Option<u64>,

    /// Minimum number of segments to always keep (default: 1)
    /// This prevents deleting all data even if retention policy says to
    pub min_segments_to_keep: u64,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_age_hours: None,
            max_segments: None,
            min_segments_to_keep: 1,
        }
    }
}

impl RetentionPolicy {
    /// Create a time-based retention policy
    pub fn time_based(max_age_hours: u64) -> Self {
        Self {
            max_age_hours: Some(max_age_hours),
            max_segments: None,
            min_segments_to_keep: 1,
        }
    }

    /// Create a size-based retention policy
    pub fn size_based(max_segments: u64) -> Self {
        Self {
            max_age_hours: None,
            max_segments: Some(max_segments),
            min_segments_to_keep: 1,
        }
    }

    /// Create a hybrid retention policy (both time and size limits)
    pub fn hybrid(max_age_hours: u64, max_segments: u64) -> Self {
        Self {
            max_age_hours: Some(max_age_hours),
            max_segments: Some(max_segments),
            min_segments_to_keep: 1,
        }
    }

    /// Check if a segment should be deleted based on retention policy
    /// Returns true if segment can be deleted
    pub fn should_delete_segment(
        &self,
        segment_age: Duration,
        total_segments: u64,
        segment_index: u64,
    ) -> bool {
        // Always keep minimum segments
        let segments_after_deletion = total_segments - (segment_index + 1);
        if segments_after_deletion < self.min_segments_to_keep {
            return false;
        }

        // Check time-based retention
        if let Some(max_age_hours) = self.max_age_hours {
            let max_age = Duration::from_secs(max_age_hours * 3600);
            if segment_age > max_age {
                return true;
            }
        }

        // Check size-based retention (keep only latest N segments)
        if let Some(max_segments) = self.max_segments {
            if total_segments > max_segments {
                // This segment is beyond the limit
                if segment_index < (total_segments - max_segments) {
                    return true;
                }
            }
        }

        false
    }

    /// Returns true if retention policy is configured (has limits)
    pub fn is_enabled(&self) -> bool {
        self.max_age_hours.is_some() || self.max_segments.is_some()
    }

    /// Get human-readable description of retention policy
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();

        if let Some(hours) = self.max_age_hours {
            if hours < 24 {
                parts.push(format!("{}h", hours));
            } else {
                parts.push(format!("{}d", hours / 24));
            }
        }

        if let Some(segments) = self.max_segments {
            parts.push(format!("{} segments", segments));
        }

        if parts.is_empty() {
            "unlimited".to_string()
        } else {
            parts.join(" or ")
        }
    }
}

/// Metadata about a segment for retention decisions
#[derive(Debug, Clone)]
pub struct SegmentInfo {
    pub segment_id: u64,
    pub created_at: SystemTime,
    pub is_sealed: bool,
}

impl SegmentInfo {
    pub fn age(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.created_at)
            .unwrap_or(Duration::from_secs(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_based_retention() {
        let policy = RetentionPolicy::time_based(24); // 24 hours

        // Old segment should be deleted
        let old_age = Duration::from_secs(25 * 3600); // 25 hours
        assert!(policy.should_delete_segment(old_age, 10, 0));

        // Recent segment should be kept
        let new_age = Duration::from_secs(1 * 3600); // 1 hour
        assert!(!policy.should_delete_segment(new_age, 10, 0));
    }

    #[test]
    fn test_size_based_retention() {
        let policy = RetentionPolicy::size_based(5); // Keep 5 segments

        // With 10 total segments, first 5 should be deleted
        assert!(policy.should_delete_segment(Duration::from_secs(0), 10, 0));
        assert!(policy.should_delete_segment(Duration::from_secs(0), 10, 4));

        // Latest 5 should be kept
        assert!(!policy.should_delete_segment(Duration::from_secs(0), 10, 5));
        assert!(!policy.should_delete_segment(Duration::from_secs(0), 10, 9));
    }

    #[test]
    fn test_min_segments_protection() {
        let policy = RetentionPolicy::time_based(1); // 1 hour

        // Even if segment is old, keep it if it's the last one
        let old_age = Duration::from_secs(100 * 3600);
        assert!(!policy.should_delete_segment(old_age, 1, 0));
    }

    #[test]
    fn test_hybrid_retention() {
        let policy = RetentionPolicy::hybrid(24, 5);

        // Old segment beyond size limit should be deleted
        let old_age = Duration::from_secs(25 * 3600);
        assert!(policy.should_delete_segment(old_age, 10, 0));

        // Recent segment within size limit should be kept
        let new_age = Duration::from_secs(1 * 3600);
        assert!(!policy.should_delete_segment(new_age, 10, 6));
    }
}
