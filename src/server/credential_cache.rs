//! Credential cache for bcrypt password verification.
//!
//! Bcrypt verification is intentionally slow (~250ms at cost 12) to resist brute-force attacks.
//! This cache stores successful authentication results for a configurable TTL to avoid
//! repeated bcrypt verification for the same credentials.
//!
//! Security considerations:
//! - Passwords are never stored in plaintext
//! - Cache keys are SHA-256 hashes of username:password
//! - Cache entries expire after the configured TTL
//! - Failed authentications are never cached

use parking_lot::RwLock;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A cache entry with expiration time
struct CacheEntry {
    /// When this entry expires
    expires_at: Instant,
}

/// Thread-safe credential cache with TTL expiration.
///
/// This cache stores SHA-256 hashes of successfully verified credentials
/// to avoid repeated bcrypt verification within the TTL window.
#[derive(Clone)]
pub struct CredentialCache {
    /// Cache of credential hashes to expiration times
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// Time-to-live for cache entries
    ttl: Duration,
}

impl CredentialCache {
    /// Create a new credential cache with the specified TTL.
    ///
    /// # Arguments
    /// * `ttl` - How long successful authentications should be cached
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            ttl,
        }
    }

    /// Generate a cache key from username and password.
    /// Uses SHA-256 to avoid storing plaintext passwords in memory.
    fn cache_key(username: &str, password: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(username.as_bytes());
        hasher.update(b":");
        hasher.update(password.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Check if credentials are cached and still valid.
    ///
    /// Returns `true` if the credentials are in the cache and haven't expired.
    pub fn is_cached(&self, username: &str, password: &str) -> bool {
        let key = Self::cache_key(username, password);
        let cache = self.cache.read();

        if let Some(entry) = cache.get(&key)
            && entry.expires_at > Instant::now()
        {
            return true;
        }
        false
    }

    /// Cache a successful authentication.
    ///
    /// Only call this after bcrypt verification succeeds.
    pub fn cache_success(&self, username: &str, password: &str) {
        let key = Self::cache_key(username, password);
        let entry = CacheEntry {
            expires_at: Instant::now() + self.ttl,
        };

        let mut cache = self.cache.write();
        cache.insert(key, entry);

        // Opportunistically clean up expired entries if cache is getting large
        if cache.len() > 1000 {
            self.cleanup_expired_internal(&mut cache);
        }
    }

    /// Remove expired entries from the cache.
    fn cleanup_expired_internal(&self, cache: &mut HashMap<String, CacheEntry>) {
        let now = Instant::now();
        cache.retain(|_, entry| entry.expires_at > now);
    }

    /// Clear all cached entries.
    ///
    /// Used when the htpasswd file is reloaded to invalidate stale credentials.
    pub fn clear(&self) {
        self.cache.write().clear();
    }

    /// Get the number of entries in the cache (for debugging/monitoring).
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.cache.read().len()
    }

    /// Check if the cache is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.cache.read().is_empty()
    }
}

impl std::fmt::Debug for CredentialCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialCache")
            .field("ttl", &self.ttl)
            .field("entries", &self.cache.read().len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_cache_hit() {
        let cache = CredentialCache::new(Duration::from_secs(60));

        // Initially not cached
        assert!(!cache.is_cached("user", "password"));

        // Cache the credentials
        cache.cache_success("user", "password");

        // Now should be cached
        assert!(cache.is_cached("user", "password"));

        // Different password should not be cached
        assert!(!cache.is_cached("user", "wrong_password"));

        // Different user should not be cached
        assert!(!cache.is_cached("other_user", "password"));
    }

    #[test]
    fn test_cache_expiry() {
        let cache = CredentialCache::new(Duration::from_millis(50));

        cache.cache_success("user", "password");
        assert!(cache.is_cached("user", "password"));

        // Wait for expiry
        sleep(Duration::from_millis(100));

        // Should no longer be cached
        assert!(!cache.is_cached("user", "password"));
    }

    #[test]
    fn test_cache_key_uniqueness() {
        // Ensure different username:password combinations produce different keys
        let key1 = CredentialCache::cache_key("user", "pass");
        let key2 = CredentialCache::cache_key("user", "pass2");
        let key3 = CredentialCache::cache_key("user2", "pass");

        assert_ne!(key1, key2);
        assert_ne!(key1, key3);
        assert_ne!(key2, key3);
    }
}
