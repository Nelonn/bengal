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
        let functions = Vec::new();

        for stmt in &statements {
            match stmt {
                Stmt::Class(class) => {
                    classes.push(class.name.clone());
                }
                Stmt::Interface(interface) => {
                    interfaces.push(interface.name.clone());
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
            if let Stmt::Import { path } = stmt {
                self.resolve_and_load(path)?;
                // Register the import in the type context
                let import_path = path.join(".");
                self.type_context.imports.push(import_path);
            }
        }
        Ok(())
    }

    pub fn build_type_context(&mut self, main_statements: &[Stmt]) -> Result<TypeContext, String> {
        self.build_type_context_with_source(main_statements, "", None)
    }

    pub fn build_type_context_with_source(&mut self, main_statements: &[Stmt], main_source: &str, main_source_path: Option<&str>) -> Result<TypeContext, String> {
        // First, process all imports
        self.process_imports(main_statements)?;

        // Load types from all loaded modules - collect statements first to avoid borrow issues
        let module_statements: Vec<(String, Vec<Stmt>)> = self.loaded_modules
            .iter()
            .map(|(name, info)| (name.clone(), info.statements.clone()))
            .collect();

        for (module_name, statements) in &module_statements {
            self.register_module_types(module_name, statements);
        }

        // Register native functions only if std.io was imported
        if self.loaded_modules.contains_key("std.io") {
            self.register_native_functions();
        }

        // Now type check all loaded modules (skip function registration since they're already registered with qualified names)
        for (module_name, statements) in &module_statements {
            let ctx = self.type_context.clone();
            let mut type_checker = TypeChecker::with_context(ctx);
            let _ = type_checker.check_with_options(statements, true);

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
                    
                    // Show code snippet if available
                    if let Some(ref source_line) = error.source_line {
                        error_msg.push_str(&format!("  {}\n", source_line));
                        // Show caret pointing to the column
                        let caret_pos = error.column.saturating_sub(1);
                        let caret_line: String = " ".repeat(caret_pos) + "^";
                        error_msg.push_str(&format!("  {}\n", caret_line));
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
                    let mut class_with_module = class.clone();
                    class_with_module.name = format!("{}.{}", module_name, class.name);
                    self.type_context.add_class(&class_with_module);
                    self.type_context.add_class(class); // Also add without module for now
                }
                Stmt::Interface(interface) => {
                    let mut interface_with_module = interface.clone();
                    interface_with_module.name = format!("{}.{}", module_name, interface.name);
                    self.type_context.add_interface(&interface_with_module);
                    self.type_context.add_interface(interface); // Also add without module for now
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
                    }).collect();

                    let full_name = format!("{}.{}", module_name, func.name);
                    self.type_context.add_function(&full_name, FunctionSignature {
                        name: full_name.clone(),
                        params,
                        return_type: func.return_type.as_ref().map(|t| Type::from_str(t)),
                        return_optional: func.return_optional,
                        is_method: false,
                        is_async: func.is_async,
                        is_native: func.is_native,
                        mangled_name: None,
                    });
                }
                Stmt::Let { name, type_annotation, expr } => {
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
                    });
                }
                _ => {}
            }
        }
    }

    fn register_native_functions(&mut self) {
        // std.io functions
        self.type_context.add_function("print", FunctionSignature {
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
            mangled_name: None,
        });

        // println with multiple overloads for different types
        self.type_context.add_function("println", FunctionSignature {
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
            mangled_name: None,
        });

        self.type_context.add_function("println", FunctionSignature {
            name: "println".to_string(),
            params: vec![ParamSignature {
                name: "line".to_string(),
                type_name: Some(Type::Int),
            }],
            return_type: None,
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
            mangled_name: None,
        });

        self.type_context.add_function("println", FunctionSignature {
            name: "println".to_string(),
            params: vec![ParamSignature {
                name: "line".to_string(),
                type_name: Some(Type::Float),
            }],
            return_type: None,
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
            mangled_name: None,
        });

        self.type_context.add_function("breakpoint", FunctionSignature {
            name: "breakpoint".to_string(),
            params: vec![],
            return_type: None,
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
            mangled_name: None,
        });

        // Register std.math native functions
        self.type_context.add_function("std.math.sin", FunctionSignature {
            name: "std.math.sin".to_string(),
            params: vec![ParamSignature {
                name: "x".to_string(),
                type_name: Some(Type::Float),
            }],
            return_type: Some(Type::Float),
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
            mangled_name: None,
        });
        self.type_context.add_function("std.math.cos", FunctionSignature {
            name: "std.math.cos".to_string(),
            params: vec![ParamSignature {
                name: "x".to_string(),
                type_name: Some(Type::Float),
            }],
            return_type: Some(Type::Float),
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
            mangled_name: None,
        });
        self.type_context.add_function("std.math.tan", FunctionSignature {
            name: "std.math.tan".to_string(),
            params: vec![ParamSignature {
                name: "x".to_string(),
                type_name: Some(Type::Float),
            }],
            return_type: Some(Type::Float),
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
            mangled_name: None,
        });
        self.type_context.add_function("std.math.sqrt", FunctionSignature {
            name: "std.math.sqrt".to_string(),
            params: vec![ParamSignature {
                name: "x".to_string(),
                type_name: Some(Type::Float),
            }],
            return_type: Some(Type::Float),
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
            mangled_name: None,
        });
        self.type_context.add_function("std.math.min", FunctionSignature {
            name: "std.math.min".to_string(),
            params: vec![ParamSignature {
                name: "a".to_string(),
                type_name: Some(Type::Float),
            }, ParamSignature {
                name: "b".to_string(),
                type_name: Some(Type::Float),
            }],
            return_type: Some(Type::Float),
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
            mangled_name: None,
        });
        self.type_context.add_function("std.math.max", FunctionSignature {
            name: "std.math.max".to_string(),
            params: vec![ParamSignature {
                name: "a".to_string(),
                type_name: Some(Type::Float),
            }, ParamSignature {
                name: "b".to_string(),
                type_name: Some(Type::Float),
            }],
            return_type: Some(Type::Float),
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
            mangled_name: None,
        });
    }

    pub fn get_loaded_modules(&self) -> &HashMap<String, ModuleInfo> {
        &self.loaded_modules
    }

    pub fn get_type_context(&self) -> &TypeContext {
        &self.type_context
    }
}
