//! Import graph for tracking module dependencies and enabling parallel processing

use std::collections::{HashMap, HashSet, VecDeque};
use rayon::prelude::*;

/// Represents the state of a module in the import graph
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleState {
    /// Module has been discovered but not yet processed
    Discovered,
    /// Module is currently being processed (used for cycle detection)
    InProgress,
    /// Module has been fully processed
    Completed,
}

/// Represents a node in the import graph
#[derive(Debug, Clone)]
pub struct ModuleNode {
    pub module_path: String,
    pub dependencies: Vec<String>,  // Modules this one imports
    pub state: ModuleState,
}

/// The import graph that tracks module dependencies
#[derive(Debug, Clone)]
pub struct ImportGraph {
    /// Map from module name to its node
    pub nodes: HashMap<String, ModuleNode>,
    /// Edges: module -> list of modules it depends on
    pub edges: HashMap<String, Vec<String>>,
    /// Reverse edges: module -> list of modules that depend on it
    pub reverse_edges: HashMap<String, Vec<String>>,
}

impl ImportGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            reverse_edges: HashMap::new(),
        }
    }

    /// Add a module to the graph
    pub fn add_module(&mut self, module_path: &str) {
        if !self.nodes.contains_key(module_path) {
            self.nodes.insert(module_path.to_string(), ModuleNode {
                module_path: module_path.to_string(),
                dependencies: Vec::new(),
                state: ModuleState::Discovered,
            });
            self.edges.entry(module_path.to_string()).or_insert_with(Vec::new);
            self.reverse_edges.entry(module_path.to_string()).or_insert_with(Vec::new);
        }
    }

    /// Add a dependency edge: `module` imports `dependency`
    pub fn add_dependency(&mut self, module: &str, dependency: &str) {
        self.add_module(module);
        self.add_module(dependency);

        if let Some(node) = self.nodes.get_mut(module) {
            if !node.dependencies.contains(&dependency.to_string()) {
                node.dependencies.push(dependency.to_string());
            }
        }

        self.edges.entry(module.to_string()).or_insert_with(Vec::new);
        if !self.edges[module].contains(&dependency.to_string()) {
            self.edges.get_mut(module).unwrap().push(dependency.to_string());
        }

        self.reverse_edges.entry(dependency.to_string()).or_insert_with(Vec::new);
        if !self.reverse_edges[dependency].contains(&module.to_string()) {
            self.reverse_edges.get_mut(dependency).unwrap().push(module.to_string());
        }
    }

    /// Detect cycles in the import graph
    /// Returns a list of modules involved in cycles
    pub fn detect_cycles(&self) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut in_stack = HashSet::new();
        let mut path = Vec::new();

        for module in self.nodes.keys() {
            if !visited.contains(module) {
                self.dfs_cycle(module, &mut visited, &mut in_stack, &mut path, &mut cycles);
            }
        }

        cycles
    }

    fn dfs_cycle(
        &self,
        module: &str,
        visited: &mut HashSet<String>,
        in_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(module.to_string());
        in_stack.insert(module.to_string());
        path.push(module.to_string());

        if let Some(dependencies) = self.edges.get(module) {
            for dep in dependencies {
                if !visited.contains(dep) {
                    self.dfs_cycle(dep, visited, in_stack, path, cycles);
                } else if in_stack.contains(dep) {
                    // Found a cycle - extract it
                    let cycle_start = path.iter().position(|m| m == dep).unwrap();
                    let cycle = path[cycle_start..].to_vec();
                    cycles.push(cycle);
                }
            }
        }

        path.pop();
        in_stack.remove(module);
    }

    /// Perform topological sort on the graph
    /// Returns modules in dependency order (dependencies first)
    /// Returns None if there are cycles
    pub fn topological_sort(&self) -> Option<Vec<Vec<String>>> {
        // Kahn's algorithm with level tracking for parallel execution
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        
        // Initialize in-degree for all nodes
        for module in self.nodes.keys() {
            in_degree.entry(module.clone()).or_insert(0);
        }

        // Calculate in-degrees
        for module in self.nodes.keys() {
            if let Some(dependencies) = self.edges.get(module) {
                for dep in dependencies {
                    *in_degree.entry(dep.clone()).or_insert(0) += 1;
                }
            }
        }

        // Wait, I got the direction wrong. Let me fix this.
        // edges: module -> dependencies (module depends on these)
        // For topological sort, we need: module -> modules that depend on it
        // So we should use reverse_edges
        
        // Recalculate with correct direction
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        
        // Initialize in-degree for all nodes
        for module in self.nodes.keys() {
            in_degree.entry(module.clone()).or_insert(0);
        }

        // Calculate in-degrees: for each module, count how many it depends on
        for module in self.nodes.keys() {
            if let Some(dependencies) = self.edges.get(module) {
                in_degree.insert(module.clone(), dependencies.len());
            }
        }

        // Start with modules that have no dependencies (in-degree 0)
        let mut queue: VecDeque<String> = VecDeque::new();
        for (module, degree) in &in_degree {
            if *degree == 0 {
                queue.push_back(module.clone());
            }
        }

        let mut levels = Vec::new();
        let mut processed = 0;

        while !queue.is_empty() {
            let level_size = queue.len();
            let mut current_level = Vec::new();

            for _ in 0..level_size {
                let module = queue.pop_front().unwrap();
                current_level.push(module.clone());
                processed += 1;

                // Find all modules that depend on this one
                if let Some(dependents) = self.reverse_edges.get(&module) {
                    for dependent in dependents {
                        let degree = in_degree.get_mut(dependent).unwrap();
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dependent.clone());
                        }
                    }
                }
            }

            if !current_level.is_empty() {
                levels.push(current_level);
            }
        }

        // If we didn't process all modules, there's a cycle
        if processed != self.nodes.len() {
            None
        } else {
            Some(levels)
        }
    }

    /// Get all modules that can be processed in parallel (no dependencies on each other)
    pub fn get_parallel_groups(&self) -> Option<Vec<Vec<String>>> {
        self.topological_sort()
    }

    /// Check if the graph has cycles
    pub fn has_cycles(&self) -> bool {
        !self.detect_cycles().is_empty()
    }

    /// Get all leaf modules (no dependencies) - these can be processed first
    pub fn get_leaf_modules(&self) -> Vec<String> {
        self.nodes.iter()
            .filter(|(_, node)| node.dependencies.is_empty())
            .map(|(path, _)| path.clone())
            .collect()
    }

    /// Parallel execution helper - process modules in levels
    /// Each level can be processed in parallel, but levels must be processed in order
    pub fn process_levels<F, R>(levels: &[Vec<String>], processor: F) -> Result<(), String>
    where
        F: Fn(&str) -> Result<R, String> + Send + Sync,
        R: Send,
    {
        for level in levels {
            // Process all modules in this level in parallel
            let results: Vec<Result<R, String>> = level.par_iter()
                .map(|module| processor(module))
                .collect();

            // Check for any errors
            for result in results {
                if let Err(e) = result {
                    return Err(e);
                }
            }
        }

        Ok(())
    }
}

impl Default for ImportGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_dependency() {
        let mut graph = ImportGraph::new();
        graph.add_dependency("main", "std.io");
        graph.add_dependency("main", "std.http");
        
        // std.io and std.http have no dependencies
        assert_eq!(graph.get_leaf_modules().len(), 2);
    }

    #[test]
    fn test_topological_sort_no_cycles() {
        let mut graph = ImportGraph::new();
        graph.add_dependency("C", "A");
        graph.add_dependency("C", "B");
        graph.add_dependency("A", "D");
        graph.add_dependency("B", "D");

        let levels = graph.topological_sort().unwrap();
        
        // D should be in first level (no dependencies)
        // A and B should be in second level
        // C should be in last level
        assert_eq!(levels.len(), 3);
        assert!(levels[0].contains(&"D".to_string()));
        assert!(levels[1].contains(&"A".to_string()));
        assert!(levels[1].contains(&"B".to_string()));
        assert!(levels[2].contains(&"C".to_string()));
    }

    #[test]
    fn test_detect_cycle() {
        let mut graph = ImportGraph::new();
        graph.add_dependency("A", "B");
        graph.add_dependency("B", "C");
        graph.add_dependency("C", "A"); // Cycle: A -> B -> C -> A

        assert!(graph.has_cycles());
        let cycles = graph.detect_cycles();
        assert!(!cycles.is_empty());
    }

    #[test]
    fn test_topological_sort_with_cycle() {
        let mut graph = ImportGraph::new();
        graph.add_dependency("A", "B");
        graph.add_dependency("B", "C");
        graph.add_dependency("C", "A");

        assert!(graph.topological_sort().is_none());
    }
}
