use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use crate::lexer::Lexer;
use crate::parser::{Parser, Stmt};
use crate::types::{TypeChecker, TypeContext, Type, FunctionSignature, ParamSignature};

pub struct ModuleResolver {
    loaded_modules: HashMap<String, ModuleInfo>,
    search_paths: Vec<PathBuf>,
    type_context: TypeContext,
}

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub path: PathBuf,
    pub statements: Vec<Stmt>,
    pub classes: Vec<String>,
    pub functions: Vec<String>,
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
        }
    }

    pub fn with_search_paths(search_paths: Vec<PathBuf>) -> Self {
        Self {
            loaded_modules: HashMap::new(),
            search_paths,
            type_context: TypeContext::new(),
        }
    }

    pub fn add_search_path(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }

    pub fn resolve_and_load(&mut self, module_path: &[String]) -> Result<(), String> {
        let module_name = module_path.join("::");
        
        // Check if already loaded
        if self.loaded_modules.contains_key(&module_name) {
            return Ok(());
        }

        // Find the module file
        let module_file = self.find_module_file(module_path)?;
        
        // Read and parse the module
        let source = fs::read_to_string(&module_file)
            .map_err(|e| format!("Failed to read module '{}': {}", module_file.display(), e))?;
        
        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize()?;
        
        let mut parser = Parser::new(tokens);
        let statements = parser.parse()?;
        
        // Create module info
        let mut classes = Vec::new();
        let functions = Vec::new();
        
        for stmt in &statements {
            match stmt {
                Stmt::Class(class) => {
                    classes.push(class.name.clone());
                }
                _ => {}
            }
        }
        
        let module_info = ModuleInfo {
            name: module_name.clone(),
            path: module_file,
            statements,
            classes,
            functions,
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
            if let Stmt::Import { path } = stmt {
                self.resolve_and_load(path)?;
            }
        }
        Ok(())
    }

    pub fn build_type_context(&mut self, main_statements: &[Stmt]) -> Result<TypeContext, String> {
        // First, process all imports
        self.process_imports(main_statements)?;

        // Load types from all loaded modules - collect statements first to avoid borrow issues
        let module_statements: Vec<(String, Vec<Stmt>)> = self.loaded_modules
            .iter()
            .map(|(name, info)| (name.clone(), info.statements.clone()))
            .collect();
        
        for (_module_name, statements) in &module_statements {
            self.register_module_types(statements);
        }

        // Register native functions from std::io
        self.register_native_functions();

        // Now type check all loaded modules
        for (module_name, statements) in &module_statements {
            let ctx = self.type_context.clone();
            let mut type_checker = TypeChecker::with_context(ctx);
            let _ = type_checker.check(statements);
            
            // Log errors but continue
            if type_checker.get_context().has_errors() {
                for error in type_checker.get_context().get_errors() {
                    eprintln!("Type error in module '{}': {}", module_name, error.message);
                }
            }
            
            // Merge the context back (including errors)
            self.type_context = type_checker.get_context().clone();
        }

        // Type check main statements
        let ctx = self.type_context.clone();
        let mut type_checker = TypeChecker::with_context(ctx);
        match type_checker.check(main_statements) {
            Ok(ctx) => Ok(ctx.clone()),
            Err(errors) => {
                let mut error_msg = String::from("Type checking failed:\n");
                for error in errors {
                    error_msg.push_str(&format!("  - {}\n", error.message));
                }
                Err(error_msg)
            }
        }
    }

    fn register_module_types(&mut self, statements: &[Stmt]) {
        for stmt in statements {
            match stmt {
                Stmt::Class(class) => {
                    self.type_context.add_class(class);
                }
                Stmt::Enum(enum_def) => {
                    self.type_context.add_enum(enum_def);
                }
                Stmt::Function(func) => {
                    let params: Vec<ParamSignature> = func.params.iter().map(|p| ParamSignature {
                        name: p.name.clone(),
                        type_name: p.type_name.as_ref().map(|t| Type::from_str(t)),
                    }).collect();

                    self.type_context.add_function(&func.name, FunctionSignature {
                        name: func.name.clone(),
                        params,
                        return_type: func.return_type.as_ref().map(|t| Type::from_str(t)),
                        return_optional: func.return_optional,
                        is_method: false,
                        is_async: func.is_async,
                        is_native: func.is_native,
                    });
                }
                _ => {}
            }
        }
    }

    fn register_native_functions(&mut self) {
        // std::io functions
        self.type_context.functions.insert("print".to_string(), FunctionSignature {
            name: "print".to_string(),
            params: vec![ParamSignature {
                name: "text".to_string(),
                type_name: Some(Type::Str),
            }],
            return_type: None,
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
        });

        self.type_context.functions.insert("println".to_string(), FunctionSignature {
            name: "println".to_string(),
            params: vec![ParamSignature {
                name: "line".to_string(),
                type_name: Some(Type::Str),
            }],
            return_type: None,
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
        });
    }

    pub fn get_loaded_modules(&self) -> &HashMap<String, ModuleInfo> {
        &self.loaded_modules
    }

    pub fn get_type_context(&self) -> &TypeContext {
        &self.type_context
    }
}
