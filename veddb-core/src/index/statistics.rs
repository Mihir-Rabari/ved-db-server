//! Index statistics tracking
//!
//! Tracks performance metrics and usage statistics for indexes

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Index statistics for performance monitoring
#[derive(Debug, Clone)]
pub struct IndexStatistics {
    /// Per-index statistics
    index_stats: HashMap<String, IndexStat>,
    /// Global statistics
    global_stats: GlobalStats,
    /// Statistics collection start time
    #[allow(dead_code)]
    start_time: Option<Instant>,
}

impl IndexStatistics {
    /// Create new index statistics
    pub fn new() -> Self {
        Self {
            index_stats: HashMap::new(),
            global_stats: GlobalStats::default(),
            start_time: Some(Instant::now()),
        }
    }

    /// Add a new index to statistics tracking
    pub fn add_index(&mut self, index_name: String) {
        self.index_stats.insert(index_name, IndexStat::default());
        self.global_stats.total_indexes += 1;
    }

    /// Remove an index from statistics tracking
    pub fn remove_index(&mut self, index_name: &str) {
        if self.index_stats.remove(index_name).is_some() {
            self.global_stats.total_indexes -= 1;
        }
    }

    /// Record an insert operation
    pub fn record_insert(&mut self) {
        self.global_stats.total_inserts += 1;
    }

    /// Record an update operation
    pub fn record_update(&mut self) {
        self.global_stats.total_updates += 1;
    }

    /// Record a delete operation
    pub fn record_delete(&mut self) {
        self.global_stats.total_deletes += 1;
    }

    /// Record a lookup operation
    pub fn record_lookup(&mut self, index_name: &str, result_count: usize) {
        if let Some(stat) = self.index_stats.get_mut(index_name) {
            stat.lookup_count += 1;
            stat.total_results += result_count as u64;
            
            if result_count > 0 {
                stat.hit_count += 1;
            } else {
                stat.miss_count += 1;
            }
        }
        
        self.global_stats.total_lookups += 1;
    }

    /// Record lookup timing
    pub fn record_lookup_time(&mut self, index_name: &str, duration: Duration) {
        if let Some(stat) = self.index_stats.get_mut(index_name) {
            stat.total_lookup_time += duration;
            
            if duration > stat.max_lookup_time {
                stat.max_lookup_time = duration;
            }
            
            if stat.min_lookup_time.is_zero() || duration < stat.min_lookup_time {
                stat.min_lookup_time = duration;
            }
        }
    }

    /// Get statistics for a specific index
    pub fn get_index_stats(&self, index_name: &str) -> Option<&IndexStat> {
        self.index_stats.get(index_name)
    }

    /// Get global statistics
    pub fn global_stats(&self) -> &GlobalStats {
        &self.global_stats
    }

    /// Get all index names
    pub fn index_names(&self) -> Vec<String> {
        self.index_stats.keys().cloned().collect()
    }

    /// Get total number of indexes
    pub fn index_count(&self) -> usize {
        self.index_stats.len()
    }

    /// Get total inserts
    pub fn total_inserts(&self) -> u64 {
        self.global_stats.total_inserts
    }

    /// Get total updates
    pub fn total_updates(&self) -> u64 {
        self.global_stats.total_updates
    }

    /// Get total deletes
    pub fn total_deletes(&self) -> u64 {
        self.global_stats.total_deletes
    }

    /// Get total lookups
    pub fn total_lookups(&self) -> u64 {
        self.global_stats.total_lookups
    }

    /// Calculate hit rate for an index
    pub fn hit_rate(&self, index_name: &str) -> f64 {
        if let Some(stat) = self.index_stats.get(index_name) {
            if stat.lookup_count > 0 {
                stat.hit_count as f64 / stat.lookup_count as f64
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Calculate average lookup time for an index
    pub fn average_lookup_time(&self, index_name: &str) -> Duration {
        if let Some(stat) = self.index_stats.get(index_name) {
            if stat.lookup_count > 0 {
                stat.total_lookup_time / stat.lookup_count as u32
            } else {
                Duration::ZERO
            }
        } else {
            Duration::ZERO
        }
    }

    /// Get uptime since statistics started
    pub fn uptime(&self) -> Duration {
        if let Some(start) = self.start_time {
            start.elapsed()
        } else {
            Duration::ZERO
        }
    }

    /// Reset all statistics
    pub fn reset(&mut self) {
        self.index_stats.clear();
        self.global_stats = GlobalStats::default();
        self.start_time = Some(Instant::now());
    }

    /// Get summary report
    pub fn summary_report(&self) -> String {
        let mut report = String::new();
        
        report.push_str(&format!("Index Statistics Summary\n"));
        report.push_str(&format!("=======================\n"));
        report.push_str(&format!("Uptime: {:?}\n", self.uptime()));
        report.push_str(&format!("Total Indexes: {}\n", self.index_count()));
        report.push_str(&format!("Total Inserts: {}\n", self.total_inserts()));
        report.push_str(&format!("Total Updates: {}\n", self.total_updates()));
        report.push_str(&format!("Total Deletes: {}\n", self.total_deletes()));
        report.push_str(&format!("Total Lookups: {}\n", self.total_lookups()));
        report.push_str(&format!("\n"));
        
        report.push_str(&format!("Per-Index Statistics:\n"));
        report.push_str(&format!("--------------------\n"));
        
        for (name, stat) in &self.index_stats {
            report.push_str(&format!("Index: {}\n", name));
            report.push_str(&format!("  Lookups: {}\n", stat.lookup_count));
            report.push_str(&format!("  Hits: {}\n", stat.hit_count));
            report.push_str(&format!("  Misses: {}\n", stat.miss_count));
            report.push_str(&format!("  Hit Rate: {:.2}%\n", self.hit_rate(name) * 100.0));
            report.push_str(&format!("  Avg Lookup Time: {:?}\n", self.average_lookup_time(name)));
            report.push_str(&format!("  Max Lookup Time: {:?}\n", stat.max_lookup_time));
            report.push_str(&format!("  Total Results: {}\n", stat.total_results));
            report.push_str(&format!("\n"));
        }
        
        report
    }
}

impl Default for IndexStatistics {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for a single index
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexStat {
    /// Number of lookup operations
    pub lookup_count: u64,
    /// Number of successful lookups (returned results)
    pub hit_count: u64,
    /// Number of unsuccessful lookups (no results)
    pub miss_count: u64,
    /// Total number of results returned
    pub total_results: u64,
    /// Total time spent on lookups
    pub total_lookup_time: Duration,
    /// Maximum lookup time
    pub max_lookup_time: Duration,
    /// Minimum lookup time
    pub min_lookup_time: Duration,
}

/// Global statistics across all indexes
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalStats {
    /// Total number of indexes
    pub total_indexes: usize,
    /// Total insert operations
    pub total_inserts: u64,
    /// Total update operations
    pub total_updates: u64,
    /// Total delete operations
    pub total_deletes: u64,
    /// Total lookup operations
    pub total_lookups: u64,
}

/// Performance metrics helper
pub struct PerformanceTimer {
    start: Instant,
}

impl PerformanceTimer {
    /// Start a new performance timer
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_index_statistics_creation() {
        let stats = IndexStatistics::new();
        assert_eq!(stats.index_count(), 0);
        assert_eq!(stats.total_inserts(), 0);
        assert_eq!(stats.total_lookups(), 0);
    }

    #[test]
    fn test_add_and_remove_index() {
        let mut stats = IndexStatistics::new();
        
        stats.add_index("test_index".to_string());
        assert_eq!(stats.index_count(), 1);
        assert!(stats.get_index_stats("test_index").is_some());
        
        stats.remove_index("test_index");
        assert_eq!(stats.index_count(), 0);
        assert!(stats.get_index_stats("test_index").is_none());
    }

    #[test]
    fn test_record_operations() {
        let mut stats = IndexStatistics::new();
        
        stats.record_insert();
        stats.record_update();
        stats.record_delete();
        
        assert_eq!(stats.total_inserts(), 1);
        assert_eq!(stats.total_updates(), 1);
        assert_eq!(stats.total_deletes(), 1);
    }

    #[test]
    fn test_record_lookups() {
        let mut stats = IndexStatistics::new();
        stats.add_index("test_index".to_string());
        
        // Record successful lookup
        stats.record_lookup("test_index", 5);
        
        // Record unsuccessful lookup
        stats.record_lookup("test_index", 0);
        
        let index_stat = stats.get_index_stats("test_index").unwrap();
        assert_eq!(index_stat.lookup_count, 2);
        assert_eq!(index_stat.hit_count, 1);
        assert_eq!(index_stat.miss_count, 1);
        assert_eq!(index_stat.total_results, 5);
        
        assert_eq!(stats.hit_rate("test_index"), 0.5);
    }

    #[test]
    fn test_lookup_timing() {
        let mut stats = IndexStatistics::new();
        stats.add_index("test_index".to_string());
        
        let duration1 = Duration::from_millis(10);
        let duration2 = Duration::from_millis(20);
        
        stats.record_lookup_time("test_index", duration1);
        stats.record_lookup_time("test_index", duration2);
        
        let index_stat = stats.get_index_stats("test_index").unwrap();
        assert_eq!(index_stat.max_lookup_time, duration2);
        assert_eq!(index_stat.min_lookup_time, duration1);
        assert_eq!(index_stat.total_lookup_time, duration1 + duration2);
    }

    #[test]
    fn test_average_lookup_time() {
        let mut stats = IndexStatistics::new();
        stats.add_index("test_index".to_string());
        
        // Record two lookups to calculate average
        stats.record_lookup("test_index", 1);
        stats.record_lookup_time("test_index", Duration::from_millis(10));
        
        stats.record_lookup("test_index", 1);
        stats.record_lookup_time("test_index", Duration::from_millis(20));
        
        let avg_time = stats.average_lookup_time("test_index");
        assert_eq!(avg_time, Duration::from_millis(15));
    }

    #[test]
    fn test_hit_rate_calculation() {
        let mut stats = IndexStatistics::new();
        stats.add_index("test_index".to_string());
        
        // 3 hits, 1 miss = 75% hit rate
        stats.record_lookup("test_index", 1); // hit
        stats.record_lookup("test_index", 2); // hit
        stats.record_lookup("test_index", 3); // hit
        stats.record_lookup("test_index", 0); // miss
        
        assert_eq!(stats.hit_rate("test_index"), 0.75);
    }

    #[test]
    fn test_summary_report() {
        let mut stats = IndexStatistics::new();
        stats.add_index("test_index".to_string());
        
        stats.record_insert();
        stats.record_lookup("test_index", 5);
        
        let report = stats.summary_report();
        assert!(report.contains("Index Statistics Summary"));
        assert!(report.contains("Total Indexes: 1"));
        assert!(report.contains("Total Inserts: 1"));
        assert!(report.contains("Index: test_index"));
    }

    #[test]
    fn test_performance_timer() {
        let timer = PerformanceTimer::start();
        sleep(Duration::from_millis(10));
        let elapsed = timer.elapsed();
        
        assert!(elapsed >= Duration::from_millis(10));
    }

    #[test]
    fn test_reset_statistics() {
        let mut stats = IndexStatistics::new();
        stats.add_index("test_index".to_string());
        stats.record_insert();
        
        assert_eq!(stats.index_count(), 1);
        assert_eq!(stats.total_inserts(), 1);
        
        stats.reset();
        
        assert_eq!(stats.index_count(), 0);
        assert_eq!(stats.total_inserts(), 0);
    }
}