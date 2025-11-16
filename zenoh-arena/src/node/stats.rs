//! Statistics tracking for node throughput

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Statistics for tracking node data throughput
#[derive(Debug, Clone)]
pub struct NodeStats {
    /// Total bytes received (input actions)
    pub input_bytes: u64,
    /// Total bytes sent (output states)
    pub output_bytes: u64,
    /// Timestamp when stats collection started
    pub start_time: Instant,
    /// Input throughput in KB/s
    pub input_kbps: f64,
    /// Output throughput in KB/s
    pub output_kbps: f64,
}

impl NodeStats {
    /// Create a new NodeStats instance with zero counters
    pub fn new() -> Self {
        Self {
            input_bytes: 0,
            output_bytes: 0,
            start_time: Instant::now(),
            input_kbps: 0.0,
            output_kbps: 0.0,
        }
    }

    /// Update throughput calculations based on elapsed time
    pub fn update_throughput(&mut self) {
        let elapsed_secs = self.start_time.elapsed().as_secs_f64();
        if elapsed_secs > 0.0 {
            self.input_kbps = (self.input_bytes as f64) / 1024.0 / elapsed_secs;
            self.output_kbps = (self.output_bytes as f64) / 1024.0 / elapsed_secs;
        }
    }

    /// Reset all statistics to zero and restart timer
    pub fn reset(&mut self) {
        self.input_bytes = 0;
        self.output_bytes = 0;
        self.start_time = Instant::now();
        self.input_kbps = 0.0;
        self.output_kbps = 0.0;
    }
}

impl Default for NodeStats {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for NodeStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Input: {:.2} KB/s ({} bytes), Output: {:.2} KB/s ({} bytes)",
            self.input_kbps, self.input_bytes, self.output_kbps, self.output_bytes
        )
    }
}

/// Thread-safe statistics tracker for node throughput
///
/// Uses atomic operations for lock-free concurrent updates
#[derive(Debug, Clone)]
pub struct StatsTracker {
    input_bytes: Arc<AtomicU64>,
    output_bytes: Arc<AtomicU64>,
    start_time: Instant,
}

impl StatsTracker {
    /// Create a new StatsTracker
    pub fn new() -> Self {
        Self {
            input_bytes: Arc::new(AtomicU64::new(0)),
            output_bytes: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
        }
    }

    /// Add input bytes to the counter
    pub fn add_input_bytes(&self, bytes: usize) {
        self.input_bytes.fetch_add(bytes as u64, Ordering::Relaxed);
    }

    /// Add output bytes to the counter
    pub fn add_output_bytes(&self, bytes: usize) {
        self.output_bytes.fetch_add(bytes as u64, Ordering::Relaxed);
    }

    /// Get current statistics snapshot
    pub fn get_stats(&self) -> NodeStats {
        let mut stats = NodeStats {
            input_bytes: self.input_bytes.load(Ordering::Relaxed),
            output_bytes: self.output_bytes.load(Ordering::Relaxed),
            start_time: self.start_time,
            input_kbps: 0.0,
            output_kbps: 0.0,
        };
        stats.update_throughput();
        stats
    }

    /// Reset statistics
    pub fn reset(&self) {
        self.input_bytes.store(0, Ordering::Relaxed);
        self.output_bytes.store(0, Ordering::Relaxed);
    }
}

impl Default for StatsTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_creation() {
        let stats = NodeStats::new();
        assert_eq!(stats.input_bytes, 0);
        assert_eq!(stats.output_bytes, 0);
        assert_eq!(stats.input_kbps, 0.0);
        assert_eq!(stats.output_kbps, 0.0);
    }

    #[test]
    fn test_stats_display() {
        let mut stats = NodeStats::new();
        stats.input_bytes = 1024;
        stats.output_bytes = 2048;
        stats.update_throughput();

        let display = format!("{}", stats);
        assert!(display.contains("Input:"));
        assert!(display.contains("Output:"));
        assert!(display.contains("KB/s"));
    }

    #[test]
    fn test_tracker_operations() {
        let tracker = StatsTracker::new();

        tracker.add_input_bytes(1024);
        tracker.add_output_bytes(2048);

        let stats = tracker.get_stats();
        assert_eq!(stats.input_bytes, 1024);
        assert_eq!(stats.output_bytes, 2048);

        tracker.reset();
        let stats = tracker.get_stats();
        assert_eq!(stats.input_bytes, 0);
        assert_eq!(stats.output_bytes, 0);
    }

    #[test]
    fn test_throughput_calculation() {
        let mut stats = NodeStats::new();
        stats.input_bytes = 10240;
        stats.output_bytes = 20480;

        // Simulate some time passing
        std::thread::sleep(std::time::Duration::from_millis(100));
        stats.update_throughput();

        assert!(stats.input_kbps > 0.0);
        assert!(stats.output_kbps > 0.0);
    }
}
