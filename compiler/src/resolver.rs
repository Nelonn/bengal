use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use crate::lexer::Lexer;
use crate::parser::{Parser, Stmt, ImportKind};
use crate::types::{TypeChecker, TypeContext, Type, FunctionSignature, ParamSignature};

pub struct ModuleResolver {
    loaded_modules: HashMap<String, ModuleInfo>,
    search_paths: Vec<PathBuf>,
    type_context: TypeContext,
    pub enable_type_checking: bool,
}

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub path: PathBuf,
    pub statements: Vec<Stmt>,
    pub classes: Vec<String>,
    pub interfaces: Vec<String>,
    pub functions: Vec<String>,
    pub source: String,
}

impl ModuleResolver {
    pub fn new() -> Self {
        let mut search_paths = Vec::new();
        
        // Add std directory to search paths - try multiple locations
        if let Ok(current_dir) = std::env::current_dir() {
            // Try current directory first
            search_paths.push(current_dir.join("std"));
            
            // Try parent directory (for when running from project root)
            if let Ok(exe_dir) = std::env::current_exe() {
                if let Some(parent) = exe_dir.parent() {
                    search_paths.push(parent.join("std"));
                }
            }
        }
        
        Self {
            loaded_modules: HashMap::new(),
            search_paths,
            type_context: TypeContext::new(),
            enable_type_checking: true,
        }
    }

    pub fn with_search_paths(search_paths: Vec<PathBuf>) -> Self {
        Self {
            loaded_modules: HashMap::new(),
            search_paths,
            type_context: TypeContext::new(),
            enable_type_checking: true,
        }
    }

    pub fn add_search_path(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }

    pub fn resolve_and_load(&mut self, module_path: &[String]) -> Result<(), String> {
        let module_name = module_path.join(".");

        // Check if already loaded
        if self.loaded_modules.contains_key(&module_name) {
            return Ok(());
        }

        // Find the module file
        let module_file = self.find_module_file(module_path)?;

        // Read and parse the module
        let source = fs::read_to_string(&module_file)
            .map_err(|e| format!("Failed to read module '{}': {}", module_file.display(), e))?;

        let mut lexer = Lexer::new(&source, module_file.to_str().unwrap_or("unknown"));
        let (tokens, token_positions) = lexer.tokenize()?;

        let mut parser = Parser::new(tokens, &source, module_file.to_str().unwrap_or("unknown"), token_positions);
        let statements = parser.parse()?;

        // Create module info
        let mut classes = Vec::new();
        let mut interfaces = Vec::new();
        let mut functions = Vec::new();

        for stmt in &statements {
            match stmt {
                Stmt::Class(class) => {
                    classes.push(class.name.clone());
                }
                Stmt::Interface(interface) => {
                    interfaces.push(interface.name.clone());
                }
                Stmt::Function(func) => {
                    functions.push(func.name.clone());
                }
                _ => {}
            }
        }

        let module_info = ModuleInfo {
            name: module_name.clone(),
            path: module_file,
            statements,
            classes,
            interfaces,
            functions,
            source: source.clone(),
        };

        self.loaded_modules.insert(module_name, module_info);

        Ok(())
    }

    fn find_module_file(&self, module_path: &[String]) -> Result<PathBuf, String> {
        // Try different file extensions
        let extensions = [".bl", ""];

        for search_path in &self.search_paths {
            for ext in &extensions {
                // If module_path starts with the search_path's last component,
                // we need to skip it to avoid duplication
                let mut file_path = search_path.clone();
                
                // Check if we need to skip the first component of module_path
                let mut start_idx = 0;
                if let Some(search_name) = search_path.file_name() {
                    if let Some(first_component) = module_path.first() {
                        if search_name.to_string_lossy() == first_component.as_str() {
                            start_idx = 1;
                        }
                    }
                }
                
                // Build path from remaining module parts
                for part in module_path.iter().skip(start_idx) {
                    file_path = file_path.join(part);
                }

                let file_with_ext = if ext.is_empty() {
                    file_path.clone()
                } else {
                    file_path.with_extension(&ext[1..])
                };

                // Check if file exists
                if file_with_ext.exists() && file_with_ext.is_file() {
                    return Ok(file_with_ext);
                }

                // Also check for directory with lib.bl inside
                let lib_path = file_path.join("lib.bl");
                if lib_path.exists() && lib_path.is_file() {
                    return Ok(lib_path);
                }
            }
        }

        Err(format!("Module '{}' not found in search paths", module_path.join("/")))
    }

    pub fn process_imports(&mut self, statements: &[Stmt]) -> Result<(), String> {
        for stmt in statements {
            if let Stmt::Import { path, kind, .. } = stmt {
                let module_name = path.join(".");

                // For wildcard imports, we need to load the module to discover its members
                // For other imports, load the module path
                let load_path = match kind {
                    ImportKind::Wildcard => {
                        // For wildcard, load the parent module
                        if path.len() > 1 {
                            path[..path.len()-1].to_vec()
                        } else {
                            path.clone()
                        }
                    }
                    ImportKind::Member => {
                        // For member imports, load the parent module
                        if path.len() > 1 {
                            path[..path.len()-1].to_vec()
                        } else {
                            path.clone()
                        }
                    }
                    ImportKind::Simple => {
                        // For simple imports, load the module itself
                        path.clone()
                    }
                    ImportKind::Module => {
                        // For module imports, load the module
                        path.clone()
                    }
                    ImportKind::Aliased(_) => {
                        // For aliased imports, load the module
                        path.clone()
                    }
                };
                
                let load_module_name = load_path.join(".");

                // Only load if not already loaded
                if !self.loaded_modules.contains_key(&load_module_name) {
                    self.resolve_and_load(&load_path)?;

                    // Recursively process imports in the loaded module
                    if let Some(info) = self.loaded_modules.get(&load_module_name) {
                        let sub_statements = info.statements.clone();
                        self.process_imports(&sub_statements)?;
                    }
                }

                // Determine the alias and module path for the import entry
                let (alias, module_path) = match kind {
                    ImportKind::Module => {
                        // import std -> module_path = "std", alias = Some("std")
                        // This allows access via std.xxx (e.g., std.io.println)
                        let alias = path.last().cloned();
                        (alias, module_name.clone())
                    }
                    ImportKind::Simple => {
                        // import std.io -> module_path = "std.io", alias = Some("io")
                        // This brings all members into scope directly (like wildcard)
                        let alias = path.last().cloned();
                        (alias, module_name.clone())
                    }
                    ImportKind::Aliased(ref alias_str) => {
                        // import std.io as myio -> module_path = "std.io", alias = Some("myio")
                        (Some(alias_str.clone()), module_name.clone())
                    }
                    ImportKind::Member => {
                        // import std.io.println -> module_path = "std.io", alias = Some("println")
                        let module_path = if path.len() > 1 {
                            path[..path.len()-1].join(".")
                        } else {
                            module_name.clone()
                        };
                        let alias = path.last().cloned();
                        (alias, module_path)
                    }
                    ImportKind::Wildcard => {
                        // import std.io.* -> module_path = "std.io", alias = None, members populated later
                        let module_path = if path.len() > 1 {
                            path[..path.len()-1].join(".")
                        } else {
                            module_name.clone()
                        };
                        (None, module_path)
                    }
                };

                // Create import entry
                let mut import_entry = crate::types::ImportEntry {
                    module_path: module_path.clone(),
                    alias,
                    kind: kind.clone(),
                    members: Vec::new(),
                };

                // For wildcard and simple imports, discover members from the loaded module
                if matches!(kind, ImportKind::Wildcard | ImportKind::Simple) {
                    if let Some(info) = self.loaded_modules.get(&module_path) {
                        // Extract function names from the module
                        for stmt in &info.statements {
                            match stmt {
                                Stmt::Function(func) => {
                                    import_entry.members.push(func.name.clone());
                                }
                                Stmt::Class(class) => {
                                    import_entry.members.push(class.name.clone());
                                }
                                Stmt::Interface(iface) => {
                                    import_entry.members.push(iface.name.clone());
                                }
                                Stmt::Let { name, .. } => {
                                    import_entry.members.push(name.clone());
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // Register the import in the type context
                self.type_context.imports.push(import_entry);
                self.type_context.import_paths.push(module_name.clone());

                // Register all members from the loaded module into the type context
                if let Some(info) = self.loaded_modules.get(&module_name) {
                    let statements = info.statements.clone();
                    self.register_module_types(&module_name, &statements);
                }
            }
        }
        Ok(())
    }

    pub fn build_type_context(&mut self, main_statements: &[Stmt]) -> Result<TypeContext, String> {
        self.build_type_context_with_source(main_statements, "", None)
    }

    pub fn build_type_context_with_source(&mut self, main_statements: &[Stmt], main_source: &str, main_source_path: Option<&str>) -> Result<TypeContext, String> {
        if !self.enable_type_checking {
            return Ok(TypeContext::new());
        }

        // First, process all imports
        self.process_imports(main_statements)?;

        // Load types from all loaded modules - collect statements first to avoid borrow issues
        let module_statements: Vec<(String, Vec<Stmt>)> = self.loaded_modules
            .iter()
            .map(|(name, info)| (name.clone(), info.statements.clone()))
            .collect();

        for (module_name, statements) in &module_statements {
            self.register_module_types(module_name, statements);
            if !self.type_context.import_paths.contains(module_name) {
                self.type_context.import_paths.push(module_name.clone());
            }
        }

        // Register native functions (always available, no import required)
        self.register_native_functions();

        // Now type check all loaded modules (skip function registration since they're already registered with qualified names)
        // Track errors from modules separately - don't merge them into the main context
        let mut module_errors: HashMap<String, Vec<(String, usize, usize)>> = HashMap::new();
        
        for (module_name, statements) in &module_statements {
            if self.enable_type_checking {
                let mut ctx = self.type_context.clone();
                ctx.current_module = Some(module_name.clone());
                let mut type_checker = TypeChecker::with_context(ctx);
                let _ = type_checker.check_with_options(statements, true);

                // Collect errors but don't merge them - we'll report them with proper file info
                if type_checker.get_context().has_errors() {
                    for error in type_checker.get_context().get_errors() {
                        module_errors
                            .entry(module_name.clone())
                            .or_insert_with(Vec::new)
                            .push((error.message.clone(), error.line, error.column));
                    }
                }

                // Merge the context back but NOT the errors (errors are reported separately)
                let mut new_ctx = type_checker.get_context().clone();
                new_ctx.errors = self.type_context.errors.clone();
                self.type_context = new_ctx;
            }
        }

        // Report all module errors with proper file paths
        for (module_name, errors) in &module_errors {
            if let Some(module_info) = self.loaded_modules.get(module_name) {
                let module_path = module_info.path.to_string_lossy().to_string();
                let source_lines: Vec<&str> = module_info.source.lines().collect();
                
                for (message, line, column) in errors {
                    let source_line = if *line > 0 && *line <= source_lines.len() {
                        source_lines[*line - 1]
                    } else {
                        ""
                    };
                    eprintln!("{}:{}:{}: error: {}", module_path, line, column, message);
                    if !source_line.is_empty() {
                        eprintln!("  {}", source_line);
                        let caret_pos = column.saturating_sub(1);
                        let caret_line: String = " ".repeat(caret_pos) + "^";
                        eprintln!("  {}", caret_line);
                    }
                }
            }
        }

        // If there were module errors, fail compilation
        if !module_errors.is_empty() {
            let total_errors: usize = module_errors.values().map(|v| v.len()).sum();
            return Err(format!("{} error(s) in imported modules", total_errors));
        }

        // Register main file's functions before type checking
        self.register_module_types("", main_statements);

        // Type check main statements
        let mut ctx = self.type_context.clone();
        ctx.current_module = None; // Clear module context for main file (global scope)
        let mut type_checker = TypeChecker::with_context(ctx);

        let result = type_checker.check(main_statements);
        
        if !self.enable_type_checking {
            let current_ctx = type_checker.get_context().clone();
            self.type_context = current_ctx.clone();
            return Ok(current_ctx);
        }

        match result {
            Ok(ctx) => {
                self.type_context = ctx.clone();
                Ok(ctx.clone())
            },
            Err(errors) => {
                let current_ctx = type_checker.get_context().clone();
                self.type_context = current_ctx.clone();
                
                if !self.enable_type_checking {
                    return Ok(current_ctx);
                }

                // If type checking is enabled, report errors and fail
                let mut error_msg = String::new();
                let source_lines: Vec<&str> = main_source.lines().collect();
                
                for mut error in errors {
                    // Set source file if not already set
                    if error.source_file.is_none() {
                        if let Some(path) = main_source_path {
                            error.source_file = Some(path.to_string());
                        }
                    }
                    
                    // Extract source line if we have line number
                    if error.source_line.is_none() && error.line > 0 && error.line <= source_lines.len() {
                        error.source_line = Some(source_lines[error.line - 1].to_string());
                    }
                    
                    // Format: FILE:LINE:COLUMN: error: message
                    let location = if let Some(ref file) = error.source_file {
                        format!("{}:{}:{}", file, error.line, error.column)
                    } else {
                        format!("{}:{}", error.line, error.column)
                    };

                    error_msg.push_str(&format!("{}: error: {}\n", location, error.message));

                    // Show code snippet if available and message is single-line
                    if let Some(ref source_line) = error.source_line {
                        // For multi-line messages (containing newlines after first line), 
                        // don't show caret as it's confusing
                        let is_multiline = error.message.contains('\n');
                        if !is_multiline {
                            error_msg.push_str(&format!("  {}\n", source_line));
                            // Show caret pointing to the column
                            let caret_pos = error.column.saturating_sub(1);
                            let caret_line: String = " ".repeat(caret_pos) + "^";
                            error_msg.push_str(&format!("  {}\n", caret_line));
                        }
                    }
                }
                Err(error_msg.trim().to_string())
            }
        }
    }

    fn register_module_types(&mut self, module_name: &str, statements: &[Stmt]) {
        for stmt in statements {
            match stmt {
                Stmt::Class(class) => {
                    self.register_class_with_module(module_name, class);
                }
                Stmt::Interface(interface) => {
                    self.register_interface_with_module(module_name, interface);
                }
                Stmt::Enum(enum_def) => {
                    self.type_context.add_enum(enum_def);
                }
                Stmt::TypeAlias(alias) => {
                    self.type_context.add_type_alias(alias);
                }
                Stmt::Function(func) => {
                    let params: Vec<ParamSignature> = func.params.iter().map(|p| ParamSignature {
                        name: p.name.clone(),
                        type_name: p.type_name.as_ref().map(|t| Type::from_str(t)),
                        default: p.default.is_some(),
                    }).collect();

                    let sig = FunctionSignature {
                        name: func.name.clone(),
                        params,
                        return_type: func.return_type.as_ref().map(|t| Type::from_str(t)),
                        return_optional: func.return_optional,
                        is_method: false,
                        is_native: func.is_native,
                        private: func.private,
                        type_params: func.type_params.clone(),
                        mangled_name: None,
                    };

                    // Register with qualified name if module_name is not empty
                    if !module_name.is_empty() {
                        let full_name = format!("{}.{}", module_name, func.name);
                        let mut sig_with_qual = sig.clone();
                        sig_with_qual.name = full_name.clone();
                        self.type_context.add_function(&full_name, sig_with_qual);
                    } else {
                        // For main file (empty module name), register without qualification
                        self.type_context.add_function(&func.name, sig.clone());
                    }

                    // Also add unqualified version for public functions in the current module
                    // (not for imported modules, to avoid name conflicts)
                    if !func.private && !module_name.is_empty() {
                        let is_current_module = self.type_context.current_module.as_ref()
                            .map(|m| m == &module_name)
                            .unwrap_or(false);
                        if is_current_module {
                            self.type_context.add_function(&func.name, sig);
                        }
                    }
                }
                Stmt::Let { name, type_annotation, expr: _, private, .. } => {
                    // Register module-level variables (e.g., math.PI)
                    let var_type = if let Some(ref ty) = type_annotation {
                        Type::from_str(ty)
                    } else {
                        // Infer type from expression (simplified - default to Unknown)
                        Type::Unknown
                    };
                    let qualified_name = format!("{}.{}", module_name, name);
                    self.type_context.variables.insert(qualified_name, crate::types::VariableInfo {
                        name: name.clone(),
                        type_name: var_type,
                        private: *private,
                    });
                }
                _ => {}
            }
        }
    }

    fn register_class_with_module(&mut self, module_name: &str, class: &crate::parser::ClassDef) {
        // Only add qualified name if module_name is not empty
        if !module_name.is_empty() {
            let mut class_with_module = class.clone();
            class_with_module.name = format!("{}.{}", module_name, class.name);
            self.type_context.add_class(&class_with_module);
        }
        
        // Always add unqualified version for classes in main file
        if module_name.is_empty() {
            self.type_context.add_class(class);
        } else if !class.private {
            // For imported modules, only add unqualified version for public classes
            self.type_context.add_class(class);
        }

        // Register nested classes with both qualified and unqualified names
        for nested_class in &class.nested_classes {
            // Full path: module.Outer.Inner
            let mut nested_full = nested_class.clone();
            nested_full.name = format!("{}.{}.{}", module_name, class.name, nested_class.name);
            self.type_context.add_class(&nested_full);
            
            // Short path: Outer.Inner (for nested lookup)
            let mut nested_short = nested_class.clone();
            nested_short.name = format!("{}.{}", class.name, nested_class.name);
            if !class.private {
                self.type_context.add_class(&nested_short);
            }
            
            // Recursively register deeper nested classes
            self.register_deeper_nested_classes(nested_class, module_name, &class.name);
        }

        // Register nested interfaces
        for nested_iface in &class.nested_interfaces {
            // Full path: module.Outer.Inner
            let mut nested_full = nested_iface.clone();
            nested_full.name = format!("{}.{}.{}", module_name, class.name, nested_iface.name);
            self.type_context.add_interface(&nested_full);
            
            // Short path: Outer.Inner (for nested lookup)
            let mut nested_short = nested_iface.clone();
            nested_short.name = format!("{}.{}", class.name, nested_iface.name);
            if !class.private {
                self.type_context.add_interface(&nested_short);
            }
        }
    }

    fn register_deeper_nested_classes(&mut self, parent_class: &crate::parser::ClassDef, module_name: &str, parent_path: &str) {
        for nested_class in &parent_class.nested_classes {
            // Full path: module.Outer.Inner.DeepInner
            let mut nested_full = nested_class.clone();
            nested_full.name = format!("{}.{}.{}", module_name, parent_path, nested_class.name);
            self.type_context.add_class(&nested_full);
            
            // Short path: Outer.Inner.DeepInner
            let mut nested_short = nested_class.clone();
            nested_short.name = format!("{}.{}", parent_path, nested_class.name);
            self.type_context.add_class(&nested_short);
            
            // Continue recursion
            self.register_deeper_nested_classes(nested_class, module_name, &format!("{}.{}", parent_path, nested_class.name));
        }
        for nested_iface in &parent_class.nested_interfaces {
            let mut nested_full = nested_iface.clone();
            nested_full.name = format!("{}.{}.{}", module_name, parent_path, nested_iface.name);
            self.type_context.add_interface(&nested_full);
            
            let mut nested_short = nested_iface.clone();
            nested_short.name = format!("{}.{}", parent_path, nested_iface.name);
            self.type_context.add_interface(&nested_short);
        }
    }

    fn register_interface_with_module(&mut self, module_name: &str, interface: &crate::parser::InterfaceDef) {
        let mut interface_with_module = interface.clone();
        interface_with_module.name = format!("{}.{}", module_name, interface.name);
        self.type_context.add_interface(&interface_with_module);
        // Only add unqualified version for public interfaces
        if !interface.private {
            self.type_context.add_interface(interface);
        }

        // Register nested classes with both qualified and unqualified names
        for nested_class in &interface.nested_classes {
            // Full path: module.Interface.NestedClass
            let mut nested_full = nested_class.clone();
            nested_full.name = format!("{}.{}.{}", module_name, interface.name, nested_class.name);
            self.type_context.add_class(&nested_full);
            
            // Short path: Interface.NestedClass (for nested lookup)
            let mut nested_short = nested_class.clone();
            nested_short.name = format!("{}.{}", interface.name, nested_class.name);
            if !interface.private {
                self.type_context.add_class(&nested_short);
            }
        }

        // Register nested interfaces
        for nested_iface in &interface.nested_interfaces {
            // Full path: module.Interface.NestedInterface
            let mut nested_full = nested_iface.clone();
            nested_full.name = format!("{}.{}.{}", module_name, interface.name, nested_iface.name);
            self.type_context.add_interface(&nested_full);
            
            // Short path: Interface.NestedInterface (for nested lookup)
            let mut nested_short = nested_iface.clone();
            nested_short.name = format!("{}.{}", interface.name, nested_iface.name);
            if !interface.private {
                self.type_context.add_interface(&nested_short);
            }
        }
    }

    fn register_native_functions(&mut self) {
        // Native functions are defined in std/*.bl files and loaded through normal import mechanism
        // No need to hardcode them here - std.math, std.test, std.io etc. define their natives in .bl files
    }

    pub fn get_loaded_modules(&self) -> &HashMap<String, ModuleInfo> {
        &self.loaded_modules
    }

    pub fn get_type_context(&self) -> &TypeContext {
        &self.type_context
    }

    pub fn get_type_context_cloned(&self) -> Option<TypeContext> {
        Some(self.type_context.clone())
    }
}
