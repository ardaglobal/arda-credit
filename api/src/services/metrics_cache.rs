//! Metrics caching system for on-demand calculation
//!
//! This module provides an in-memory cache for metrics data to avoid
//! repeated database-intensive calculations. It supports:
//! - Composite cache keys for different query parameters
//! - TTL-based expiration for cache entries
//! - Cache statistics and management
//! - Thread-safe operations with RwLock

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};
use uuid::Uuid;

use super::{MarketValueDataPoint, PieChartDataPoint};

/// Configuration for the metrics cache
#[derive(Debug, Clone)]
pub struct MetricsCacheConfig {
    /// Time-to-live for cache entries (default: 1 hour)
    pub ttl: Duration,
    /// Maximum number of entries before cleanup (default: 10000)
    pub max_entries: usize,
    /// Enable cache statistics collection (default: true)
    pub enable_stats: bool,
}

impl Default for MetricsCacheConfig {
    fn default() -> Self {
        Self {
            ttl: Duration::from_secs(3600), // 1 hour
            max_entries: 10000,
            enable_stats: true,
        }
    }
}

/// Cache key for market value metrics
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MarketValueCacheKey {
    pub growth_type: String,
    pub filter_by: String,
    pub entity_id: Option<Uuid>,
    pub time_range: String,
}

impl MarketValueCacheKey {
    pub fn new(
        growth_type: String,
        filter_by: String,
        entity_id: Option<Uuid>,
        time_range: String,
    ) -> Self {
        Self {
            growth_type,
            filter_by,
            entity_id,
            time_range,
        }
    }
}

impl std::fmt::Display for MarketValueCacheKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}",
            self.growth_type,
            self.filter_by,
            self.entity_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "none".to_string()),
            self.time_range
        )
    }
}

/// Cache key for pie chart metrics
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PieChartCacheKey {
    pub chart_type: String,
    pub filter_by: String,
    pub entity_id: Option<Uuid>,
    pub time_range: String,
}

impl PieChartCacheKey {
    pub fn new(
        chart_type: String,
        filter_by: String,
        entity_id: Option<Uuid>,
        time_range: String,
    ) -> Self {
        Self {
            chart_type,
            filter_by,
            entity_id,
            time_range,
        }
    }
}

impl std::fmt::Display for PieChartCacheKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}",
            self.chart_type,
            self.filter_by,
            self.entity_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "none".to_string()),
            self.time_range
        )
    }
}

/// Cached entry with expiration
#[derive(Debug, Clone)]
struct CachedEntry<T> {
    data: T,
    created_at: Instant,
    ttl: Duration,
}

impl<T> CachedEntry<T> {
    fn new(data: T, ttl: Duration) -> Self {
        Self {
            data,
            created_at: Instant::now(),
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
}

/// Cache statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub market_value_hits: u64,
    pub market_value_misses: u64,
    pub pie_chart_hits: u64,
    pub pie_chart_misses: u64,
    pub total_entries: usize,
    pub cleanup_runs: u64,
    pub entries_evicted: u64,
}

impl CacheStats {
    /// Get market value hit ratio as percentage
    pub fn market_value_hit_ratio(&self) -> f64 {
        let total = self.market_value_hits + self.market_value_misses;
        if total == 0 {
            0.0
        } else {
            (self.market_value_hits as f64 / total as f64) * 100.0
        }
    }

    /// Get pie chart hit ratio as percentage
    pub fn pie_chart_hit_ratio(&self) -> f64 {
        let total = self.pie_chart_hits + self.pie_chart_misses;
        if total == 0 {
            0.0
        } else {
            (self.pie_chart_hits as f64 / total as f64) * 100.0
        }
    }

    /// Get overall hit ratio as percentage
    pub fn overall_hit_ratio(&self) -> f64 {
        let total_hits = self.market_value_hits + self.pie_chart_hits;
        let total_requests = total_hits + self.market_value_misses + self.pie_chart_misses;
        if total_requests == 0 {
            0.0
        } else {
            (total_hits as f64 / total_requests as f64) * 100.0
        }
    }
}

/// In-memory cache for metrics data
#[derive(Debug)]
pub struct MetricsCache {
    market_value_cache:
        RwLock<HashMap<MarketValueCacheKey, CachedEntry<Vec<MarketValueDataPoint>>>>,
    pie_chart_cache: RwLock<HashMap<PieChartCacheKey, CachedEntry<Vec<PieChartDataPoint>>>>,
    stats: RwLock<CacheStats>,
    config: MetricsCacheConfig,
}

impl MetricsCache {
    /// Create a new metrics cache with the given configuration
    pub fn new(config: MetricsCacheConfig) -> Self {
        info!(
            "Initializing metrics cache with TTL: {:?}, max_entries: {}",
            config.ttl, config.max_entries
        );

        Self {
            market_value_cache: RwLock::new(HashMap::new()),
            pie_chart_cache: RwLock::new(HashMap::new()),
            stats: RwLock::new(CacheStats::default()),
            config,
        }
    }

    /// Create a new metrics cache with default configuration
    pub fn default() -> Self {
        Self::new(MetricsCacheConfig::default())
    }

    // ========================================================================
    // MARKET VALUE CACHE OPERATIONS
    // ========================================================================

    /// Get market value data from cache
    #[instrument(skip(self), level = "debug")]
    pub async fn get_market_value(
        &self,
        key: &MarketValueCacheKey,
    ) -> Option<Vec<MarketValueDataPoint>> {
        let cache = self.market_value_cache.read().await;

        if let Some(entry) = cache.get(key) {
            if !entry.is_expired() {
                debug!("Cache HIT for market value key: {}", key);

                if self.config.enable_stats {
                    let mut stats = self.stats.write().await;
                    stats.market_value_hits += 1;
                }

                return Some(entry.data.clone());
            } else {
                debug!("Cache EXPIRED for market value key: {}", key);
            }
        }

        debug!("Cache MISS for market value key: {}", key);

        if self.config.enable_stats {
            let mut stats = self.stats.write().await;
            stats.market_value_misses += 1;
        }

        None
    }

    /// Store market value data in cache
    #[instrument(skip(self, data), level = "debug")]
    pub async fn put_market_value(
        &self,
        key: MarketValueCacheKey,
        data: Vec<MarketValueDataPoint>,
    ) {
        debug!("Storing market value data in cache for key: {}", key);

        let entry = CachedEntry::new(data, self.config.ttl);
        let mut cache = self.market_value_cache.write().await;
        cache.insert(key, entry);

        // Update stats
        if self.config.enable_stats {
            let mut stats = self.stats.write().await;
            stats.total_entries = cache.len()
                + self
                    .pie_chart_cache
                    .try_read()
                    .map(|c| c.len())
                    .unwrap_or(0);
        }

        // Check if cleanup is needed
        if cache.len() > self.config.max_entries {
            drop(cache);
            self.cleanup_expired_entries().await;
        }
    }

    // ========================================================================
    // PIE CHART CACHE OPERATIONS
    // ========================================================================

    /// Get pie chart data from cache
    #[instrument(skip(self), level = "debug")]
    pub async fn get_pie_chart(&self, key: &PieChartCacheKey) -> Option<Vec<PieChartDataPoint>> {
        let cache = self.pie_chart_cache.read().await;

        if let Some(entry) = cache.get(key) {
            if !entry.is_expired() {
                debug!("Cache HIT for pie chart key: {}", key);

                if self.config.enable_stats {
                    let mut stats = self.stats.write().await;
                    stats.pie_chart_hits += 1;
                }

                return Some(entry.data.clone());
            } else {
                debug!("Cache EXPIRED for pie chart key: {}", key);
            }
        }

        debug!("Cache MISS for pie chart key: {}", key);

        if self.config.enable_stats {
            let mut stats = self.stats.write().await;
            stats.pie_chart_misses += 1;
        }

        None
    }

    /// Store pie chart data in cache
    #[instrument(skip(self, data), level = "debug")]
    pub async fn put_pie_chart(&self, key: PieChartCacheKey, data: Vec<PieChartDataPoint>) {
        debug!("Storing pie chart data in cache for key: {}", key);

        let entry = CachedEntry::new(data, self.config.ttl);
        let mut cache = self.pie_chart_cache.write().await;
        cache.insert(key, entry);

        // Update stats
        if self.config.enable_stats {
            let mut stats = self.stats.write().await;
            stats.total_entries = cache.len()
                + self
                    .market_value_cache
                    .try_read()
                    .map(|c| c.len())
                    .unwrap_or(0);
        }

        // Check if cleanup is needed
        if cache.len() > self.config.max_entries {
            drop(cache);
            self.cleanup_expired_entries().await;
        }
    }

    // ========================================================================
    // CACHE MANAGEMENT
    // ========================================================================

    /// Clean up expired entries from both caches
    #[instrument(skip(self), level = "info")]
    pub async fn cleanup_expired_entries(&self) {
        info!("Starting cache cleanup");

        let mut evicted_count = 0;

        // Clean up market value cache
        {
            let mut cache = self.market_value_cache.write().await;
            let initial_count = cache.len();
            cache.retain(|key, entry| {
                if entry.is_expired() {
                    debug!("Evicting expired market value entry: {}", key);
                    false
                } else {
                    true
                }
            });
            evicted_count += initial_count - cache.len();
        }

        // Clean up pie chart cache
        {
            let mut cache = self.pie_chart_cache.write().await;
            let initial_count = cache.len();
            cache.retain(|key, entry| {
                if entry.is_expired() {
                    debug!("Evicting expired pie chart entry: {}", key);
                    false
                } else {
                    true
                }
            });
            evicted_count += initial_count - cache.len();
        }

        // Update stats
        if self.config.enable_stats {
            let mut stats = self.stats.write().await;
            stats.cleanup_runs += 1;
            stats.entries_evicted += evicted_count as u64;
            stats.total_entries = self
                .market_value_cache
                .try_read()
                .map(|c| c.len())
                .unwrap_or(0)
                + self
                    .pie_chart_cache
                    .try_read()
                    .map(|c| c.len())
                    .unwrap_or(0);
        }

        info!("Cache cleanup completed. Evicted {} entries", evicted_count);
    }

    /// Invalidate all cache entries (force refresh)
    #[instrument(skip(self), level = "info")]
    pub async fn invalidate_all(&self) {
        info!("Invalidating all cache entries");

        let mut market_cache = self.market_value_cache.write().await;
        let mut pie_cache = self.pie_chart_cache.write().await;

        let total_evicted = market_cache.len() + pie_cache.len();

        market_cache.clear();
        pie_cache.clear();

        if self.config.enable_stats {
            let mut stats = self.stats.write().await;
            stats.entries_evicted += total_evicted as u64;
            stats.total_entries = 0;
        }

        info!("Invalidated {} cache entries", total_evicted);
    }

    /// Invalidate cache entries for a specific entity
    #[instrument(skip(self), level = "info")]
    pub async fn invalidate_entity(&self, entity_id: Uuid) {
        info!("Invalidating cache entries for entity: {}", entity_id);

        let mut evicted_count = 0;

        // Invalidate market value entries for this entity
        {
            let mut cache = self.market_value_cache.write().await;
            let initial_count = cache.len();
            cache.retain(|key, _| key.entity_id != Some(entity_id));
            evicted_count += initial_count - cache.len();
        }

        // Invalidate pie chart entries for this entity
        {
            let mut cache = self.pie_chart_cache.write().await;
            let initial_count = cache.len();
            cache.retain(|key, _| key.entity_id != Some(entity_id));
            evicted_count += initial_count - cache.len();
        }

        if self.config.enable_stats {
            let mut stats = self.stats.write().await;
            stats.entries_evicted += evicted_count as u64;
            stats.total_entries = self
                .market_value_cache
                .try_read()
                .map(|c| c.len())
                .unwrap_or(0)
                + self
                    .pie_chart_cache
                    .try_read()
                    .map(|c| c.len())
                    .unwrap_or(0);
        }

        info!(
            "Invalidated {} cache entries for entity: {}",
            evicted_count, entity_id
        );
    }

    /// Get cache statistics
    #[instrument(skip(self), level = "debug")]
    pub async fn get_stats(&self) -> CacheStats {
        if self.config.enable_stats {
            self.stats.read().await.clone()
        } else {
            CacheStats::default()
        }
    }

    /// Get cache configuration
    pub fn get_config(&self) -> &MetricsCacheConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::time::Duration;

    #[tokio::test]
    async fn test_market_value_cache_operations() {
        let cache = MetricsCache::default();
        let key = MarketValueCacheKey::new(
            "arda_growth".to_string(),
            "originator".to_string(),
            Some(Uuid::new_v4()),
            "all_time".to_string(),
        );
        let data = vec![MarketValueDataPoint {
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            cumulative_value: "100.00".to_string(),
            deal_count: 1,
        }];

        // Test miss
        assert!(cache.get_market_value(&key).await.is_none());

        // Test put and hit
        cache.put_market_value(key.clone(), data.clone()).await;
        let cached_data = cache.get_market_value(&key).await;
        assert!(cached_data.is_some());
        assert_eq!(cached_data.unwrap().len(), 1);

        // Test stats
        let stats = cache.get_stats().await;
        assert_eq!(stats.market_value_hits, 1);
        assert_eq!(stats.market_value_misses, 1);
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let config = MetricsCacheConfig {
            ttl: Duration::from_millis(10), // Very short TTL for testing
            max_entries: 100,
            enable_stats: true,
        };
        let cache = MetricsCache::new(config);

        let key = MarketValueCacheKey::new(
            "arda_growth".to_string(),
            "all".to_string(),
            None,
            "all_time".to_string(),
        );
        let data = vec![MarketValueDataPoint {
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            cumulative_value: "100.00".to_string(),
            deal_count: 1,
        }];

        // Put data and immediately get it
        cache.put_market_value(key.clone(), data.clone()).await;
        assert!(cache.get_market_value(&key).await.is_some());

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Should be expired now
        assert!(cache.get_market_value(&key).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let cache = MetricsCache::default();
        let entity_id = Uuid::new_v4();

        let key1 = MarketValueCacheKey::new(
            "arda_growth".to_string(),
            "originator".to_string(),
            Some(entity_id),
            "all_time".to_string(),
        );
        let key2 = MarketValueCacheKey::new(
            "arda_growth".to_string(),
            "originator".to_string(),
            Some(Uuid::new_v4()),
            "all_time".to_string(),
        );

        let data = vec![MarketValueDataPoint {
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            cumulative_value: "100.00".to_string(),
            deal_count: 1,
        }];

        // Put data for both entities
        cache.put_market_value(key1.clone(), data.clone()).await;
        cache.put_market_value(key2.clone(), data.clone()).await;

        // Both should be cached
        assert!(cache.get_market_value(&key1).await.is_some());
        assert!(cache.get_market_value(&key2).await.is_some());

        // Invalidate one entity
        cache.invalidate_entity(entity_id).await;

        // First should be gone, second should remain
        assert!(cache.get_market_value(&key1).await.is_none());
        assert!(cache.get_market_value(&key2).await.is_some());
    }
}
