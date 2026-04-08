//! Compilation cache for storing and retrieving compiled modules
//!
//! This module provides disk-based caching of compiled HLIR modules to enable
//! incremental compilation. Each cache entry stores the compiled module along
//! with metadata to detect when recompilation is needed.

use crate::hlir::{HlirModule, HlirClass, HlirFunction};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use sha2::{Sha256, Digest};

/// Metadata about a compiled module for cache validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleMetadata {
    /// Module path (e.g., "std.io")
    pub module_path: String,
    /// Absolute file path to the source file
    pub source_path: String,
    /// SHA256 hash of source file content
    pub source_hash: String,
    /// File modification time (Unix timestamp)
    pub mtime: u64,
    /// Compilation timestamp
    pub compiled_at: u64,
    /// Compiler version for compatibility
    pub compiler_version: String,
}

/// A compiled module ready for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledModule {
    /// Module metadata for cache validation
    pub metadata: ModuleMetadata,
    /// The HLIR representation
    pub hlir: HlirModule,
    /// Import dependencies (module paths this module imports)
    pub imports: Vec<String>,
    /// Exported functions
    pub exported_functions: Vec<String>,
    /// Exported classes
    pub exported_classes: Vec<String>,
}

/// Cache entry with serialized data
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    module: CompiledModule,
}

/// Cache manager for handling disk-based caching
pub struct CacheManager {
    /// Cache directory path
    cache_dir: PathBuf,
    /// Whether caching is enabled
    enabled: bool,
    /// Compiler version for cache compatibility
    compiler_version: String,
}

impl CacheManager {
    /// Create a new cache manager
    pub fn new<P: AsRef<Path>>(cache_dir: P, enabled: bool) -> Self {
        let cache_manager = Self {
            cache_dir: cache_dir.as_ref().to_path_buf(),
            enabled,
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        };

        // Create cache directory if it doesn't exist
        if enabled {
            if let Err(e) = fs::create_dir_all(&cache_manager.cache_dir) {
                eprintln!("Warning: Failed to create cache directory: {}", e);
            }
        }

        cache_manager
    }

    /// Check if caching is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Compute SHA256 hash of source content
    fn compute_source_hash(source: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(source.as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)
    }

    /// Get file modification time
    fn get_file_mtime(path: &Path) -> u64 {
        fs::metadata(path)
            .and_then(|meta| meta.modified())
            .map(|time| {
                time.duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    }

    /// Get current timestamp
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Generate cache key from module path and source hash
    fn generate_cache_key(module_path: &str, source_hash: &str) -> String {
        format!("{}_{}", module_path.replace('.', "_"), source_hash)
    }

    /// Get cache file path for a module
    fn get_cache_file_path(&self, module_path: &str, source_hash: &str) -> PathBuf {
        let key = Self::generate_cache_key(module_path, source_hash);
        self.cache_dir.join(format!("{}.bin", key))
    }

    /// Create a CompiledModule from HLIR and metadata
    pub fn create_compiled_module(
        module_path: &str,
        source_path: &str,
        source: &str,
        hlir: HlirModule,
        imports: Vec<String>,
    ) -> CompiledModule {
        let source_hash = Self::compute_source_hash(source);
        let mtime = Self::get_file_mtime(Path::new(source_path));

        // Extract exported functions and classes from HLIR
        let exported_functions: Vec<String> = hlir.functions.iter()
            .filter(|f| !f.name.starts_with('_')) // Skip internal functions like _main
            .map(|f| f.name.clone())
            .collect();

        let exported_classes: Vec<String> = hlir.classes.iter()
            .map(|c| c.name.clone())
            .collect();

        let metadata = ModuleMetadata {
            module_path: module_path.to_string(),
            source_path: source_path.to_string(),
            source_hash,
            mtime,
            compiled_at: Self::current_timestamp(),
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        };

        CompiledModule {
            metadata,
            hlir,
            imports,
            exported_functions,
            exported_classes,
        }
    }

    /// Save a compiled module to the cache
    pub fn save_to_cache(&self, module: &CompiledModule) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        let cache_path = self.get_cache_file_path(
            &module.metadata.module_path,
            &module.metadata.source_hash,
        );

        // Serialize to binary format using bincode for efficiency
        let entry = CacheEntry {
            module: module.clone(),
        };

        let data = match bincode::serialize(&entry) {
            Ok(d) => d,
            Err(e) => return Err(format!("Failed to serialize cache entry: {}", e)),
        };

        if let Err(e) = fs::write(&cache_path, data) {
            return Err(format!("Failed to write cache file: {}", e));
        }

        Ok(())
    }

    /// Try to load a compiled module from cache
    pub fn load_from_cache(
        &self,
        module_path: &str,
        source_path: &str,
        source: &str,
    ) -> Option<CompiledModule> {
        if !self.enabled {
            return None;
        }

        let current_hash = Self::compute_source_hash(source);
        let cache_path = self.get_cache_file_path(module_path, &current_hash);

        // Check if cache file exists
        if !cache_path.exists() {
            return None;
        }

        // Read and deserialize
        let data = match fs::read(&cache_path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Warning: Failed to read cache file: {}", e);
                return None;
            }
        };

        let entry: CacheEntry = match bincode::deserialize(&data) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Warning: Failed to deserialize cache entry: {}", e);
                return None;
            }
        };

        // Validate cache entry
        if !self.validate_cache_entry(&entry.module, source_path, &current_hash) {
            return None;
        }

        Some(entry.module)
    }

    /// Validate a cache entry to ensure it's still valid
    fn validate_cache_entry(
        &self,
        module: &CompiledModule,
        source_path: &str,
        current_hash: &str,
    ) -> bool {
        // Check compiler version compatibility
        if module.metadata.compiler_version != self.compiler_version {
            return false;
        }

        // Check source hash matches
        if module.metadata.source_hash != current_hash {
            return false;
        }

        // Check file modification time hasn't changed
        let current_mtime = Self::get_file_mtime(Path::new(source_path));
        if current_mtime != module.metadata.mtime {
            return false;
        }

        true
    }

    /// Clear the entire cache
    pub fn clear_cache(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        if self.cache_dir.exists() {
            if let Err(e) = fs::remove_dir_all(&self.cache_dir) {
                return Err(format!("Failed to clear cache: {}", e));
            }
        }

        Ok(())
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> CacheStats {
        let mut stats = CacheStats {
            total_entries: 0,
            total_size_bytes: 0,
            modules: HashMap::new(),
        };

        if !self.cache_dir.exists() {
            return stats;
        }

        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    stats.total_entries += 1;
                    stats.total_size_bytes += metadata.len();

                    if let Some(name) = entry.file_name().to_str() {
                        if let Some(module_name) = name.split('_').next() {
                            *stats.modules.entry(module_name.to_string()).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        stats
    }
}

/// Cache statistics
#[derive(Debug, Default)]
pub struct CacheStats {
    pub total_entries: usize,
    pub total_size_bytes: u64,
    pub modules: HashMap<String, usize>,
}

impl std::fmt::Display for CacheStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Cache Statistics:")?;
        writeln!(f, "  Total entries: {}", self.total_entries)?;
        writeln!(f, "  Total size: {:.2} MB", self.total_size_bytes as f64 / 1_048_576.0)?;
        writeln!(f, "  Modules cached:")?;
        for (module, count) in &self.modules {
            writeln!(f, "    {}: {} entries", module, count)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hlir::{HlirFunction, HlirBasicBlock, HlirType};

    fn create_test_hlir_module() -> HlirModule {
        HlirModule {
            name: "test".to_string(),
            functions: vec![HlirFunction::new(
                "test_func".to_string(),
                vec![("x".to_string(), HlirType::I32)],
                HlirType::I32,
            )],
            globals: vec![],
            classes: vec![],
        }
    }

    #[test]
    fn test_cache_save_and_load() {
        let cache_dir = std::env::temp_dir().join("bengal_test_cache");
        let cache = CacheManager::new(&cache_dir, true);

        let source = "fn test(x: int): int { return x + 1; }";
        let hlir = create_test_hlir_module();

        let compiled = CacheManager::create_compiled_module(
            "test",
            "test.bl",
            source,
            hlir.clone(),
            vec![],
        );

        // Save to cache
        assert!(cache.save_to_cache(&compiled).is_ok());

        // Load from cache
        let loaded = cache.load_from_cache("test", "test.bl", source);
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.metadata.module_path, "test");
        assert_eq!(loaded.hlir.functions.len(), 1);

        // Clean up
        let _ = cache.clear_cache();
    }

    #[test]
    fn test_cache_invalidates_on_change() {
        let cache_dir = std::env::temp_dir().join("bengal_test_cache_invalid");
        let cache = CacheManager::new(&cache_dir, true);

        let source_v1 = "fn test(x: int): int { return x + 1; }";
        let source_v2 = "fn test(x: int): int { return x + 2; }";

        let hlir = create_test_hlir_module();
        let compiled = CacheManager::create_compiled_module(
            "test",
            "test.bl",
            source_v1,
            hlir.clone(),
            vec![],
        );

        cache.save_to_cache(&compiled).unwrap();

        // Try to load with different source - should fail
        let loaded = cache.load_from_cache("test", "test.bl", source_v2);
        assert!(loaded.is_none());

        // Load with same source - should succeed
        let loaded = cache.load_from_cache("test", "test.bl", source_v1);
        assert!(loaded.is_some());

        let _ = cache.clear_cache();
    }

    #[test]
    fn test_cache_disabled() {
        let cache_dir = std::env::temp_dir().join("bengal_test_cache_disabled");
        let cache = CacheManager::new(&cache_dir, false);

        let source = "fn test(x: int): int { return x; }";
        let hlir = create_test_hlir_module();

        let compiled = CacheManager::create_compiled_module(
            "test",
            "test.bl",
            source,
            hlir,
            vec![],
        );

        // Should not save when disabled
        cache.save_to_cache(&compiled).unwrap();
        assert!(!cache_dir.exists());

        // Should not load when disabled
        let loaded = cache.load_from_cache("test", "test.bl", source);
        assert!(loaded.is_none());
    }
}
