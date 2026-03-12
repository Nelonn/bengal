//! Runtime linker for native functions with indexed lookup and hot-swap support
//! 
//! This module provides:
//! - O(1) indexed native function lookup instead of HashMap string lookups
//! - Dynamic runtime linking for hot-swap of native functions
//! - Function registration with automatic index assignment

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::vm::NativeFn;

/// Maximum number of native functions that can be registered
pub const MAX_NATIVE_FUNCTIONS: usize = 65535;  // u16::MAX

/// A native function entry with metadata for hot-swap support
#[derive(Clone)]
pub struct NativeFunctionEntry {
    /// The function pointer
    pub func: NativeFn,
    /// The function name for debugging and lookup
    pub name: String,
    /// Parameter count (optional, for validation)
    pub param_count: Option<usize>,
    /// Whether this function can be hot-swapped
    pub hot_swappable: bool,
    /// Version number for tracking updates
    pub version: u64,
}

/// Registry for native functions using indexed lookup
/// 
/// Instead of HashMap<String, NativeFn>, we use:
/// - Vec<NativeFunctionEntry> for O(1) indexed access
/// - HashMap<String, u16> for name-to-index mapping (only during linking)
#[derive(Clone)]
pub struct NativeFunctionRegistry {
    /// Indexed array of native functions for O(1) lookup
    functions: Vec<Option<NativeFunctionEntry>>,
    /// Name to index mapping for registration and linking
    name_to_index: HashMap<String, u16>,
    /// Fallback function for unknown natives
    fallback: Option<NativeFn>,
    /// Version counter for tracking changes
    version: u64,
    /// Callback when functions are updated (for relinking)
    on_update: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Default for NativeFunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeFunctionRegistry {
    /// Create a new native function registry
    pub fn new() -> Self {
        Self {
            functions: Vec::with_capacity(256),
            name_to_index: HashMap::new(),
            fallback: None,
            version: 0,
            on_update: None,
        }
    }

    /// Set the callback to be invoked when functions are updated
    pub fn set_update_callback(&mut self, callback: Arc<dyn Fn() + Send + Sync>) {
        self.on_update = Some(callback);
    }

    /// Register a native function and return its index
    /// 
    /// Returns the index that should be used for calls
    pub fn register(&mut self, name: &str, func: NativeFn) -> u16 {
        if let Some(&index) = self.name_to_index.get(name) {
            // Function already exists, update it (hot-swap)
            if let Some(Some(entry)) = self.functions.get_mut(index as usize) {
                entry.func = func;
                entry.version = self.version;
                self.version += 1;
                if let Some(ref callback) = self.on_update {
                    callback();
                }
                return index;
            }
        }

        // New function, assign an index
        let index = self.functions.len();
        if index >= MAX_NATIVE_FUNCTIONS {
            panic!("Too many native functions registered (max: {}). Last function: {}", MAX_NATIVE_FUNCTIONS, name);
        }

        self.name_to_index.insert(name.to_string(), index as u16);
        self.functions.push(Some(NativeFunctionEntry {
            func,
            name: name.to_string(),
            param_count: None,
            hot_swappable: true,
            version: self.version,
        }));
        self.version += 1;

        index as u16
    }

    /// Register a native function with metadata
    pub fn register_with_metadata(
        &mut self,
        name: &str,
        func: NativeFn,
        param_count: Option<usize>,
        hot_swappable: bool,
    ) -> u16 {
        if let Some(&index) = self.name_to_index.get(name) {
            if let Some(Some(entry)) = self.functions.get_mut(index as usize) {
                entry.func = func;
                entry.param_count = param_count;
                entry.hot_swappable = hot_swappable;
                entry.version = self.version;
                self.version += 1;
                if let Some(ref callback) = self.on_update {
                    callback();
                }
                return index;
            }
        }

        let index = self.functions.len() as u16;
        if index >= MAX_NATIVE_FUNCTIONS as u16 {
            panic!("Too many native functions registered (max: {})", MAX_NATIVE_FUNCTIONS);
        }

        self.name_to_index.insert(name.to_string(), index);
        self.functions.push(Some(NativeFunctionEntry {
            func,
            name: name.to_string(),
            param_count,
            hot_swappable,
            version: self.version,
        }));
        self.version += 1;

        index
    }

    /// Get a native function by index (O(1) operation)
    pub fn get_by_index(&self, index: u16) -> Option<NativeFn> {
        self.functions
            .get(index as usize)
            .and_then(|entry| entry.as_ref().map(|e| e.func))
    }

    /// Get a native function entry by index with metadata
    pub fn get_entry(&self, index: u16) -> Option<&NativeFunctionEntry> {
        self.functions.get(index as usize).and_then(|e| e.as_ref())
    }

    /// Get the index for a function name (for linking phase)
    pub fn get_index(&self, name: &str) -> Option<u16> {
        self.name_to_index.get(name).copied()
    }

    /// Set the fallback function
    pub fn set_fallback(&mut self, func: NativeFn) {
        self.fallback = Some(func);
    }

    /// Get the fallback function
    pub fn get_fallback(&self) -> Option<NativeFn> {
        self.fallback
    }

    /// Hot-swap a native function (replace implementation at runtime)
    /// 
    /// Returns true if the function was successfully swapped
    pub fn hot_swap(&mut self, name: &str, new_func: NativeFn) -> bool {
        if let Some(&index) = self.name_to_index.get(name) {
            if let Some(Some(entry)) = self.functions.get_mut(index as usize) {
                if entry.hot_swappable {
                    entry.func = new_func;
                    entry.version = self.version;
                    self.version += 1;
                    if let Some(ref callback) = self.on_update {
                        callback();
                    }
                    return true;
                }
            }
        }
        false
    }

    /// Get the current version (incremented on each change)
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Get the number of registered functions
    pub fn len(&self) -> usize {
        self.functions.iter().filter(|e| e.is_some()).count()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all registered functions
    pub fn clear(&mut self) {
        self.functions.clear();
        self.name_to_index.clear();
        self.version += 1;
    }

    /// Unregister a function by name
    pub fn unregister(&mut self, name: &str) -> bool {
        if let Some(index) = self.name_to_index.remove(name) {
            if let Some(entry) = self.functions.get_mut(index as usize) {
                *entry = None;
                self.version += 1;
                return true;
            }
        }
        false
    }
}

/// Runtime linker that manages the linking between bytecode and native functions
///
/// Supports:
/// - Initial linking: Map string names to indices
/// - Hot-swap: Update function pointers without recompilation
/// - Relinking: Update bytecode indices when functions change
#[derive(Clone)]
pub struct RuntimeLinker {
    /// The native function registry
    registry: Arc<RwLock<NativeFunctionRegistry>>,
    /// Cached bytecode patches for fast relinking
    /// TODO: Implement relinking feature
    #[allow(dead_code)]
    bytecode_patches: HashMap<String, Vec<PatchLocation>>,
    /// Current linked version
    linked_version: u64,
}

/// A location in bytecode that needs to be patched when a function changes
#[derive(Clone, Debug)]
pub struct PatchLocation {
    /// Bytecode data reference (weak)
    pub bytecode_id: usize,
    /// Offset in the bytecode where the index is stored
    pub offset: usize,
    /// The function name this patch is for
    pub function_name: String,
}

impl Default for RuntimeLinker {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeLinker {
    /// Create a new runtime linker
    pub fn new() -> Self {
        Self {
            registry: Arc::new(RwLock::new(NativeFunctionRegistry::new())),
            bytecode_patches: HashMap::new(),
            linked_version: 0,
        }
    }

    /// Create a new runtime linker with a shared registry
    pub fn with_registry(registry: Arc<RwLock<NativeFunctionRegistry>>) -> Self {
        Self {
            registry,
            bytecode_patches: HashMap::new(),
            linked_version: 0,
        }
    }

    /// Get a reference to the registry
    pub fn registry(&self) -> Arc<RwLock<NativeFunctionRegistry>> {
        Arc::clone(&self.registry)
    }

    /// Register a native function
    pub fn register(&mut self, name: &str, func: NativeFn) -> u16 {
        let mut registry = self.registry.write().unwrap();
        registry.register(name, func)
    }

    /// Register a native function with metadata
    pub fn register_with_metadata(
        &mut self,
        name: &str,
        func: NativeFn,
        param_count: Option<usize>,
        hot_swappable: bool,
    ) -> u16 {
        let mut registry = self.registry.write().unwrap();
        registry.register_with_metadata(name, func, param_count, hot_swappable)
    }

    /// Set the fallback function
    pub fn set_fallback(&mut self, func: NativeFn) {
        let mut registry = self.registry.write().unwrap();
        registry.set_fallback(func);
    }

    /// Link bytecode to native functions
    /// 
    /// This converts string-based native calls to indexed calls
    pub fn link_bytecode(&mut self, bytecode: &mut [u8], strings: &[String]) -> Vec<PatchLocation> {
        use crate::vm::Opcode;
        
        let mut patches = Vec::new();
        let registry = self.registry.read().unwrap();
        
        let mut i = 0;
        while i < bytecode.len() {
            let opcode = bytecode[i];
            
            // Handle CallNative opcode
            if opcode == Opcode::CallNative as u8 {
                // Format: [CallNative, Rd, name_idx, arg_start, arg_count]
                if i + 4 < bytecode.len() {
                    let _rd = bytecode[i + 1];
                    let name_idx = bytecode[i + 2] as usize;

                    if let Some(name) = strings.get(name_idx) {
                        if let Some(func_index) = registry.get_index(name) {
                            // Replace string index with function index
                            // We'll use a new opcode CallNativeIndexed that takes u16 index
                            bytecode[i + 2] = (func_index & 0xFF) as u8;
                            bytecode[i + 3] = ((func_index >> 8) & 0xFF) as u8;

                            patches.push(PatchLocation {
                                bytecode_id: 0, // TODO: Use actual bytecode ID
                                offset: i + 2,
                                function_name: name.clone(),
                            });
                        }
                    }
                }
            }
            
            // Move to next instruction
            i += 1;
        }
        
        self.linked_version = registry.version();
        patches
    }

    /// Hot-swap a native function
    pub fn hot_swap(&mut self, name: &str, new_func: NativeFn) -> bool {
        let mut registry = self.registry.write().unwrap();
        registry.hot_swap(name, new_func)
    }

    /// Check if relinking is needed
    pub fn needs_relinking(&self) -> bool {
        let registry = self.registry.read().unwrap();
        registry.version() != self.linked_version
    }

    /// Get the current version
    pub fn version(&self) -> u64 {
        let registry = self.registry.read().unwrap();
        registry.version()
    }
}

#[cfg(test)]
mod tests {
    use crate::Value;
    use super::*;

    fn dummy_native(_args: &mut Vec<Value>) -> Result<Value, Value> {
        Ok(Value::Null)
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = NativeFunctionRegistry::new();
        let index = registry.register("test::func", dummy_native);
        
        assert_eq!(index, 0);
        assert!(registry.get_by_index(index).is_some());
        assert_eq!(registry.get_index("test::func"), Some(0));
    }

    #[test]
    fn test_hot_swap() {
        let mut registry = NativeFunctionRegistry::new();
        registry.register("test::func", dummy_native);
        
        fn new_native(_args: &mut Vec<Value>) -> Result<Value, Value> {
            Ok(Value::Int64(42))
        }
        
        assert!(registry.hot_swap("test::func", new_native));
        
        // Verify the function was swapped
        let entry = registry.get_entry(0).unwrap();
        assert_eq!(entry.version, 1);
    }

    #[test]
    fn test_version_tracking() {
        let mut registry = NativeFunctionRegistry::new();
        assert_eq!(registry.version(), 0);
        
        registry.register("test::func1", dummy_native);
        assert_eq!(registry.version(), 1);
        
        registry.register("test::func2", dummy_native);
        assert_eq!(registry.version(), 2);
        
        registry.hot_swap("test::func1", dummy_native);
        assert_eq!(registry.version(), 3);
    }
}
