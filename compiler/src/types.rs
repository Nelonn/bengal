use std::collections::HashMap;
use crate::parser::{ClassDef, Method, Expr, Literal, Stmt, CastType};

fn is_numeric_type(ty: &Type) -> bool {
    matches!(ty, Type::Int | Type::Float)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    Str,
    Bool,
    Class(String),
    Enum(String),
    Optional(Box<Type>),
    Promise(Box<Type>),
    Null,
    Unknown,
}

impl Type {
    pub fn from_str(s: &str) -> Self {
        // Handle optional type suffix
        if s.ends_with('?') {
            let inner = s.trim_end_matches('?');
            return Type::Optional(Box::new(Type::from_str(inner)));
        }
        
        match s {
            "int" => Type::Int,
            "float" => Type::Float,
            "str" => Type::Str,
            "bool" => Type::Bool,
            _ => Type::Class(s.to_string()),
        }
    }

    pub fn to_str(&self) -> String {
        match self {
            Type::Int => "int".to_string(),
            Type::Float => "float".to_string(),
            Type::Str => "str".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Class(name) => name.clone(),
            Type::Enum(name) => name.clone(),
            Type::Optional(t) => format!("{}?", t.to_str()),
            Type::Promise(t) => format!("Promise<{}>", t.to_str()),
            Type::Null => "null".to_string(),
            Type::Unknown => "unknown".to_string(),
        }
    }

    pub fn is_assignable_to(&self, other: &Type) -> bool {
        match (self, other) {
            (Type::Null, Type::Optional(_)) => true,
            (Type::Null, Type::Promise(_)) => true,
            (inner, Type::Optional(target)) => inner.is_assignable_to(target),
            (Type::Optional(inner), other) => inner.is_assignable_to(other),
            (a, b) if a == b => true,
            (Type::Int, Type::Float) => true,
            (_, Type::Unknown) => true,
            (Type::Unknown, _) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub name: String,
    pub params: Vec<ParamSignature>,
    pub return_type: Option<Type>,
    pub return_optional: bool,
    pub is_method: bool,
    pub is_async: bool,
    pub is_native: bool,
}

#[derive(Debug, Clone)]
pub struct ParamSignature {
    pub name: String,
    pub type_name: Option<Type>,
}

#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub name: String,
    pub fields: HashMap<String, FieldInfo>,
    pub methods: HashMap<String, MethodSignature>,
    pub is_native: bool,
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub type_name: Type,
    pub private: bool,
}

#[derive(Debug, Clone)]
pub struct MethodSignature {
    pub name: String,
    pub params: Vec<ParamSignature>,
    pub return_type: Option<Type>,
    pub return_optional: bool,
    pub private: bool,
    pub is_async: bool,
    pub is_native: bool,
}

#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub name: String,
    pub variants: HashMap<String, EnumVariantInfo>,
}

#[derive(Debug, Clone)]
pub struct EnumVariantInfo {
    pub name: String,
    pub value: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct VariableInfo {
    pub name: String,
    pub type_name: Type,
}

#[derive(Debug, Clone)]
pub struct TypeContext {
    pub classes: HashMap<String, ClassInfo>,
    pub functions: HashMap<String, FunctionSignature>,
    pub variables: HashMap<String, VariableInfo>,
    pub enums: HashMap<String, EnumInfo>,
    pub current_class: Option<String>,
    pub current_method_return: Option<Type>,
    pub current_async_inner_return: Option<Type>,
    pub current_method_params: Vec<String>,
    pub imports: Vec<String>,
    pub errors: Vec<TypeError>,
}

#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub source_file: Option<String>,
    pub source_line: Option<String>,
}

impl TypeContext {
    pub fn new() -> Self {
        let mut ctx = Self {
            classes: HashMap::new(),
            functions: HashMap::new(),
            variables: HashMap::new(),
            enums: HashMap::new(),
            current_class: None,
            current_method_return: None,
            current_async_inner_return: None,
            current_method_params: Vec::new(),
            imports: Vec::new(),
            errors: Vec::new(),
        };

        // Register native classes
        ctx.register_native_classes();

        ctx
    }

    fn register_native_classes(&mut self) {
        // Register std::io module functions
        let io_print = FunctionSignature {
            name: "print".to_string(),
            params: vec![ParamSignature {
                name: "text".to_string(),
                type_name: Some(Type::Unknown),
            }],
            return_type: None,
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
        };

        let io_println = FunctionSignature {
            name: "println".to_string(),
            params: vec![ParamSignature {
                name: "line".to_string(),
                type_name: Some(Type::Unknown),
            }],
            return_type: None,
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
        };

        self.functions.insert("print".to_string(), io_print);
        self.functions.insert("println".to_string(), io_println);

        // JSON
        let json_stringify = FunctionSignature {
            name: "std::json::stringify".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
            }],
            return_type: Some(Type::Str),
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
        };

        let json_parse = FunctionSignature {
            name: "std::json::parse".to_string(),
            params: vec![ParamSignature {
                name: "json".to_string(),
                type_name: Some(Type::Str),
            }],
            return_type: Some(Type::Unknown),
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
        };

        self.functions.insert("std::json::stringify".to_string(), json_stringify);
        self.functions.insert("std::json::parse".to_string(), json_parse);
        self.imports.push("std::json".to_string());

        // Reflection
        let reflect_typeof = FunctionSignature {
            name: "std::reflect::type_of".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
            }],
            return_type: Some(Type::Str),
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
        };

        let reflect_class_name = FunctionSignature {
            name: "std::reflect::class_name".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
            }],
            return_type: Some(Type::Str),
            return_optional: true,
            is_method: false,
            is_async: false,
            is_native: true,
        };

        let reflect_fields = FunctionSignature {
            name: "std::reflect::fields".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
            }],
            return_type: Some(Type::Unknown),
            return_optional: true,
            is_method: false,
            is_async: false,
            is_native: true,
        };

        self.functions.insert("std::reflect::type_of".to_string(), reflect_typeof);
        self.functions.insert("std::reflect::class_name".to_string(), reflect_class_name);
        self.functions.insert("std::reflect::fields".to_string(), reflect_fields);
        self.imports.push("std::reflect".to_string());

        // Mark std::io as imported by default for native functions
        self.imports.push("std::io".to_string());
    }

    pub fn add_class(&mut self, class: &ClassDef) {
        let mut fields = HashMap::new();
        for field in &class.fields {
            fields.insert(field.name.clone(), FieldInfo {
                name: field.name.clone(),
                type_name: Type::from_str(&field.type_name),
                private: field.private,
            });
        }

        let mut methods = HashMap::new();
        for method in &class.methods {
            let params: Vec<ParamSignature> = method.params.iter().map(|p| ParamSignature {
                name: p.name.clone(),
                type_name: p.type_name.as_ref().map(|t| Type::from_str(t)),
            }).collect();

            methods.insert(method.name.clone(), MethodSignature {
                name: method.name.clone(),
                params,
                return_type: method.return_type.as_ref().map(|t| Type::from_str(t)),
                return_optional: method.return_optional,
                private: method.private,
                is_async: method.is_async,
                is_native: method.is_native,
            });
        }

        self.classes.insert(class.name.clone(), ClassInfo {
            name: class.name.clone(),
            fields,
            methods,
            is_native: false,
        });
    }

    pub fn add_function(&mut self, name: &str, signature: FunctionSignature) {
        self.functions.insert(name.to_string(), signature);
    }

    pub fn add_enum(&mut self, enum_def: &crate::parser::EnumDef) {
        let mut variants = HashMap::new();
        let mut next_value: i64 = 0;

        for variant in &enum_def.variants {
            let value = if let Some(expr) = &variant.value {
                if let Expr::Literal(Literal::Int(n)) = expr {
                    next_value = *n + 1;
                    Some(*n)
                } else {
                    next_value += 1;
                    None
                }
            } else {
                let v = Some(next_value);
                next_value += 1;
                v
            };

            variants.insert(variant.name.clone(), EnumVariantInfo {
                name: variant.name.clone(),
                value,
            });
        }

        self.enums.insert(enum_def.name.clone(), EnumInfo {
            name: enum_def.name.clone(),
            variants,
        });
    }

    pub fn get_enum(&self, name: &str) -> Option<&EnumInfo> {
        self.enums.get(name)
    }

    pub fn get_enum_variant(&self, enum_name: &str, variant_name: &str) -> Option<&EnumVariantInfo> {
        self.enums.get(enum_name).and_then(|e| e.variants.get(variant_name))
    }

    pub fn add_variable(&mut self, name: &str, type_name: Type) {
        self.variables.insert(name.to_string(), VariableInfo {
            name: name.to_string(),
            type_name,
        });
    }

    pub fn get_variable(&self, name: &str) -> Option<&VariableInfo> {
        self.variables.get(name)
    }

    pub fn get_class(&self, name: &str) -> Option<&ClassInfo> {
        self.classes.get(name)
    }

    pub fn get_function(&self, name: &str) -> Option<&FunctionSignature> {
        self.functions.get(name)
    }

    /// Try to resolve a function name, including searching for unqualified names
    /// in qualified functions (e.g., "foo" matches "std::sys::foo")
    pub fn resolve_function(&self, name: &str) -> Option<&FunctionSignature> {
        // First try exact match
        if let Some(sig) = self.functions.get(name) {
            return Some(sig);
        }

        // Try to find a function that ends with ::<name>
        // Prefer exact module match if we're in a module context
        for (func_name, sig) in &self.functions {
            if func_name.ends_with(&format!("::{}", name)) {
                return Some(sig);
            }
        }

        None
    }

    /// Try to resolve a class name, including searching for unqualified names
    /// in qualified classes (e.g., "HttpClient" matches "std::http::HttpClient")
    pub fn resolve_class(&self, name: &str) -> Option<String> {
        // First try exact match
        if self.classes.contains_key(name) {
            // If the exact match contains ::, use it; otherwise check if there's also a qualified version
            let exact = name.to_string();
            if exact.contains("::") {
                return Some(exact);
            }
            // Prefer qualified version if available
            for class_name in self.classes.keys() {
                if class_name.ends_with(&format!("::{}", name)) {
                    return Some(class_name.clone());
                }
            }
            return Some(exact);
        }

        // Try to find a class that ends with ::<name>
        for class_name in self.classes.keys() {
            if class_name.ends_with(&format!("::{}", name)) {
                return Some(class_name.clone());
            }
        }

        None
    }

    pub fn get_method(&self, class_name: &str, method_name: &str) -> Option<&MethodSignature> {
        self.classes.get(class_name).and_then(|c| c.methods.get(method_name))
    }

    pub fn add_error(&mut self, message: String, line: usize) {
        self.errors.push(TypeError {
            message,
            line,
            column: 0,
            source_file: None,
            source_line: None,
        });
    }

    pub fn add_error_with_location(&mut self, message: String, line: usize, column: usize, source_file: Option<String>, source_line: Option<String>) {
        self.errors.push(TypeError {
            message,
            line,
            column,
            source_file,
            source_line,
        });
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn get_errors(&self) -> &[TypeError] {
        &self.errors
    }
}

pub struct TypeChecker {
    context: TypeContext,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            context: TypeContext::new(),
        }
    }

    pub fn with_context(context: TypeContext) -> Self {
        Self { context }
    }

    pub fn check(&mut self, statements: &[Stmt]) -> Result<&TypeContext, Vec<TypeError>> {
        self.check_with_options(statements, false)
    }

    /// Type check statements, optionally skipping function registration
    /// (for imported modules where functions are already registered with qualified names)
    pub fn check_with_options(&mut self, statements: &[Stmt], skip_functions: bool) -> Result<&TypeContext, Vec<TypeError>> {
        // First pass: collect all class and function definitions
        self.collect_definitions(statements, skip_functions);

        // Second pass: type check all statements
        for stmt in statements {
            self.check_stmt(stmt);
        }

        if self.context.has_errors() {
            Err(self.context.errors.clone())
        } else {
            Ok(&self.context)
        }
    }

    pub fn get_context(&self) -> &TypeContext {
        &self.context
    }

    pub fn get_context_mut(&mut self) -> &mut TypeContext {
        &mut self.context
    }

    fn collect_definitions(&mut self, statements: &[Stmt], skip_functions: bool) {
        for stmt in statements {
            match stmt {
                Stmt::Class(class) => {
                    self.context.add_class(class);
                }
                Stmt::Enum(enum_def) => {
                    self.context.add_enum(enum_def);
                }
                Stmt::Function(func) => {
                    if !skip_functions {
                        let params: Vec<ParamSignature> = func.params.iter().map(|p| ParamSignature {
                            name: p.name.clone(),
                            type_name: p.type_name.as_ref().map(|t| Type::from_str(t)),
                        }).collect();

                        self.context.add_function(&func.name, FunctionSignature {
                            name: func.name.clone(),
                            params,
                            return_type: func.return_type.as_ref().map(|t| Type::from_str(t)),
                            return_optional: func.return_optional,
                            is_method: false,
                            is_async: func.is_async,
                            is_native: func.is_native,
                        });
                    }
                }
                Stmt::Import { path: _ } => {
                    // Import handled during module resolution
                }
                _ => {}
            }
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Module { path: _ } => {
                // Module declaration - just for namespacing
            }
            Stmt::Import { path: _ } => {
                // Import handled in collect_definitions
            }
            Stmt::Class(class) => {
                self.check_class(class);
            }
            Stmt::Enum(_) => {
                // Enum definitions are already processed in collect_definitions
            }
            Stmt::Function(func) => {
                self.check_function(func);
            }
            Stmt::Let { name, expr } => {
                let expr_type = self.infer_expr(expr);
                self.context.add_variable(name, expr_type);
            }
            Stmt::Assign { name, expr, span } => {
                let expr_type = self.infer_expr(expr);

                if let Some(var_info) = self.context.get_variable(name) {
                    if !expr_type.is_assignable_to(&var_info.type_name) {
                        self.context.add_error(
                            format!(
                                "Type mismatch: cannot assign {} to variable '{}' of type {}",
                                expr_type.to_str(),
                                name,
                                var_info.type_name.to_str()
                            ),
                            0
                        );
                    }
                } else if let Some(current_class) = &self.context.current_class {
                    // Check if assigning to a class field without self
                    if let Some(class_info) = self.context.get_class(current_class) {
                        if class_info.fields.contains_key(name) {
                            // Error: assigning to class member without self
                            self.context.add_error_with_location(
                                format!(
                                    "Cannot assign to class member '{}' without 'self' keyword. Use 'self.{}' instead.",
                                    name, name
                                ),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                            return;
                        }
                    }
                    // Inside a class method, implicit variable declarations are not allowed
                    self.context.add_error_with_location(
                        format!("Undeclared variable '{}'. Use 'let {} = ...' to declare a local variable", name, name),
                        span.line,
                        span.column,
                        None,
                        None,
                    );
                } else {
                    // Variable not declared with let, create it (implicit declaration in global/function scope)
                    self.context.add_variable(name, expr_type);
                }
            }
            Stmt::Return(expr) => {
                // For async functions, check against the inner return type
                let expected_return = if self.context.current_async_inner_return.is_some() {
                    self.context.current_async_inner_return.clone()
                } else {
                    self.context.current_method_return.clone()
                };

                if let Some(expected) = &expected_return {
                    if let Some(e) = expr {
                        let expr_type = self.infer_expr(e);
                        if !expr_type.is_assignable_to(expected) {
                            self.context.add_error(
                                format!(
                                    "Return type mismatch: expected {}, got {}",
                                    expected.to_str(),
                                    expr_type.to_str()
                                ),
                                0
                            );
                        }
                    } else if !matches!(expected, Type::Null | Type::Unknown) {
                        self.context.add_error(
                            format!(
                                "Expected return value of type {}, but no value returned",
                                expected.to_str()
                            ),
                            0
                        );
                    }
                }
            }
            Stmt::Expr(expr) => {
                self.infer_expr(expr);
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let cond_type = self.infer_expr(condition);
                if cond_type != Type::Bool && cond_type != Type::Unknown {
                    self.context.add_error(
                        format!("Expected bool condition, got {}", cond_type.to_str()),
                        0
                    );
                }
                
                for stmt in then_branch {
                    self.check_stmt(stmt);
                }
                
                if let Some(else_b) = else_branch {
                    for stmt in else_b {
                        self.check_stmt(stmt);
                    }
                }
            }
            Stmt::For { var_name, range, body } => {
                let _range_type = self.infer_expr(range);
                // For now, assume ranges are integers
                self.context.add_variable(var_name, Type::Int);
                for stmt in body {
                    self.check_stmt(stmt);
                }
                self.context.variables.remove(var_name);
            }
            Stmt::While { condition, body } => {
                let cond_type = self.infer_expr(condition);
                if cond_type != Type::Bool && cond_type != Type::Unknown {
                    self.context.add_error(
                        format!("Expected bool condition for while, got {}", cond_type.to_str()),
                        0
                    );
                }
                for stmt in body {
                    self.check_stmt(stmt);
                }
            }
            Stmt::TryCatch { try_block, catch_var, catch_block } => {
                for stmt in try_block {
                    self.check_stmt(stmt);
                }
                
                // Add catch variable (exception object) - currently unknown type
                self.context.add_variable(catch_var, Type::Unknown);
                
                for stmt in catch_block {
                    self.check_stmt(stmt);
                }
                
                self.context.variables.remove(catch_var);
            }
            Stmt::Throw(expr) => {
                self.infer_expr(expr);
            }
            Stmt::Break => {
                // Break statement - no type checking needed
            }
        }
    }

    fn check_class(&mut self, class: &ClassDef) {
        let old_class = self.context.current_class.clone();
        self.context.current_class = Some(class.name.clone());

        for method in &class.methods {
            self.check_method(method, &class.name);
        }

        self.context.current_class = old_class;
    }

    fn check_function(&mut self, func: &crate::parser::FunctionDef) {
        let old_return = self.context.current_method_return.clone();
        let old_async_inner = self.context.current_async_inner_return.clone();

        // Handle optional return types
        let mut return_type = func.return_type.as_ref().map(|t| {
            let ty = Type::from_str(t);
            if func.return_optional {
                Type::Optional(Box::new(ty))
            } else {
                ty
            }
        });

        // Async functions return Promise<T> but inner return is T
        if func.is_async {
            let inner_type = return_type.clone().unwrap_or(Type::Null);
            self.context.current_async_inner_return = Some(inner_type.clone());
            return_type = Some(Type::Promise(Box::new(inner_type)));
        }

        self.context.current_method_return = return_type;

        // Add parameters as local variables
        let mut added_vars = Vec::new();
        for param in &func.params {
            let param_type = param.type_name.as_ref()
                .map(|t| Type::from_str(t))
                .unwrap_or(Type::Unknown);
            self.context.add_variable(&param.name, param_type.clone());
            added_vars.push(param.name.clone());
        }

        // Check function body
        for stmt in &func.body {
            self.check_stmt(stmt);
        }

        // Clean up local variables
        for var in added_vars {
            self.context.variables.remove(&var);
        }

        self.context.current_method_return = old_return;
        self.context.current_async_inner_return = old_async_inner;
    }

    fn check_method(&mut self, method: &Method, class_name: &str) {
        let old_return = self.context.current_method_return.clone();
        let old_async_inner = self.context.current_async_inner_return.clone();

        // Handle optional return types
        let mut return_type = method.return_type.as_ref().map(|t| {
            let ty = Type::from_str(t);
            if method.return_optional {
                Type::Optional(Box::new(ty))
            } else {
                ty
            }
        });

        // Async methods return Promise<T> but inner return is T
        if method.is_async {
            let inner_type = return_type.clone().unwrap_or(Type::Null);
            self.context.current_async_inner_return = Some(inner_type.clone());
            return_type = Some(Type::Promise(Box::new(inner_type)));
        }

        self.context.current_method_return = return_type;

        // Add parameters as local variables
        let mut added_vars = Vec::new();
        for param in &method.params {
            let param_type = param.type_name.as_ref()
                .map(|t| Type::from_str(t))
                .unwrap_or(Type::Unknown);
            self.context.add_variable(&param.name, param_type.clone());
            added_vars.push(param.name.clone());
        }

        // Add 'self' variable
        self.context.add_variable("self", Type::Class(class_name.to_string()));

        // Check method body
        for stmt in &method.body {
            self.check_stmt(stmt);
        }

        // Clean up local variables
        for var in added_vars {
            self.context.variables.remove(&var);
        }
        self.context.variables.remove("self");

        self.context.current_method_return = old_return;
        self.context.current_async_inner_return = old_async_inner;
    }

    fn infer_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Literal(lit) => {
                match lit {
                    Literal::String(_) => Type::Str,
                    Literal::Int(_) => Type::Int,
                    Literal::Float(_) => Type::Float,
                    Literal::Bool(_) => Type::Bool,
                    Literal::Null => Type::Null,
                }
            }
            Expr::Variable { name, span } => {
                if let Some(var_info) = self.context.get_variable(name) {
                    var_info.type_name.clone()
                } else if let Some(current_class) = &self.context.current_class {
                    if let Some(class_info) = self.context.get_class(current_class) {
                        if let Some(field_info) = class_info.fields.get(name) {
                            // Error: accessing class member without self
                            let field_type = field_info.type_name.clone();
                            self.context.add_error_with_location(
                                format!(
                                    "Cannot access class member '{}' without 'self' keyword. Use 'self.{}' instead.",
                                    name, name
                                ),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                            return field_type;
                        }
                    }
                    // Variable not found in class context - report as undeclared
                    self.context.add_error_with_location(
                        format!("Undeclared variable '{}'", name),
                        span.line,
                        span.column,
                        None,
                        None,
                    );
                    Type::Unknown
                } else if self.context.get_enum(name).is_some() {
                    // Enum type access
                    Type::Enum(name.clone())
                } else {
                    // Variable not found in global/function context - report as undeclared
                    self.context.add_error_with_location(
                        format!("Undeclared variable '{}'", name),
                        span.line,
                        span.column,
                        None,
                        None,
                    );
                    Type::Unknown
                }
            }
            Expr::Binary { left, op, right, .. } => {
                let left_type = self.infer_expr(left);
                let right_type = self.infer_expr(right);

                // Type checking for binary operations
                match op {
                    crate::parser::BinaryOp::Equal | crate::parser::BinaryOp::NotEqual => {
                        // Equality can be checked between any types, but they should match
                        if left_type != right_type &&
                           left_type != Type::Unknown &&
                           right_type != Type::Unknown {
                            self.context.add_error(
                                format!(
                                    "Cannot compare {} with {} using equality operator",
                                    left_type.to_str(),
                                    right_type.to_str()
                                ),
                                0
                            );
                        }
                        Type::Bool
                    }
                    crate::parser::BinaryOp::And | crate::parser::BinaryOp::Or => {
                        if left_type != Type::Bool && left_type != Type::Unknown {
                            self.context.add_error(
                                format!("Expected bool for logical operator, got {}", left_type.to_str()),
                                0
                            );
                        }
                        if right_type != Type::Bool && right_type != Type::Unknown {
                            self.context.add_error(
                                format!("Expected bool for logical operator, got {}", right_type.to_str()),
                                0
                            );
                        }
                        Type::Bool
                    }
                    crate::parser::BinaryOp::Add | crate::parser::BinaryOp::Subtract |
                    crate::parser::BinaryOp::Multiply | crate::parser::BinaryOp::Divide => {
                        if op == &crate::parser::BinaryOp::Add && (left_type == Type::Str || right_type == Type::Str) {
                            return Type::Str;
                        }

                        // Arithmetic operations require numeric types
                        if !is_numeric_type(&left_type) && left_type != Type::Unknown {
                            self.context.add_error(
                                format!("Expected numeric type for arithmetic operation, got {}", left_type.to_str()),
                                0
                            );
                        }
                        if !is_numeric_type(&right_type) && right_type != Type::Unknown {
                            self.context.add_error(
                                format!("Expected numeric type for arithmetic operation, got {}", right_type.to_str()),
                                0
                            );
                        }
                        // Result type is the more precise type (float > int)
                        if left_type == Type::Float || right_type == Type::Float {
                            Type::Float
                        } else {
                            Type::Int
                        }
                    }
                    crate::parser::BinaryOp::Greater | crate::parser::BinaryOp::Less => {
                        // Comparison operations require numeric types and return bool
                        if !is_numeric_type(&left_type) && left_type != Type::Unknown {
                            self.context.add_error(
                                format!("Expected numeric type for comparison, got {}", left_type.to_str()),
                                0
                            );
                        }
                        if !is_numeric_type(&right_type) && right_type != Type::Unknown {
                            self.context.add_error(
                                format!("Expected numeric type for comparison, got {}", right_type.to_str()),
                                0
                            );
                        }
                        Type::Bool
                    }
                }
            }
            Expr::Unary { op, expr, .. } => {
                let inner_type = self.infer_expr(expr);
                match op {
                    crate::parser::UnaryOp::Not => {
                        if inner_type != Type::Bool && inner_type != Type::Unknown {
                            self.context.add_error(
                                format!("Expected bool for ! operator, got {}", inner_type.to_str()),
                                0
                            );
                        }
                        Type::Bool
                    }
                    crate::parser::UnaryOp::Decrement => {
                        // Decrement operator works on numeric types and returns the original type
                        if inner_type != Type::Int && inner_type != Type::Float && inner_type != Type::Unknown {
                            self.context.add_error(
                                format!("Expected numeric type for -- operator, got {}", inner_type.to_str()),
                                0
                            );
                        }
                        inner_type
                    }
                }
            }
            Expr::Call { callee, args, .. } => {
                if let Expr::Variable { name: func_name, .. } = callee.as_ref() {
                    // Check if it's a function call
                    let func_sig = self.context.get_function(func_name).cloned();
                    if let Some(ref sig) = func_sig {
                        self.check_function_call(sig, args, func_name);
                        let mut return_type = sig.return_type.clone().unwrap_or(Type::Unknown);
                        // If calling an async function, return type is Promise<T>
                        if sig.is_async {
                            if let Type::Promise(_) = return_type {
                                // Already a Promise type
                            } else {
                                return_type = Type::Promise(Box::new(return_type));
                            }
                        }
                        return_type
                    } else if let Some(class_info) = self.context.get_class(func_name) {
                        // It's a class instantiation - check constructor args
                        let ctor_sig = class_info.methods.get("__init__").cloned();
                        if let Some(ref sig) = ctor_sig {
                            self.check_method_call(sig, args, "__init__", func_name);
                        } else if !args.is_empty() {
                            // No explicit constructor but args were provided
                            self.context.add_error(
                                format!("Class '{}' does not accept constructor arguments", func_name),
                                0
                            );
                        }
                        // Return type is the class type
                        Type::Class(func_name.clone())
                    } else {
                        Type::Unknown
                    }
                } else if let Expr::Get { object, name, .. } = callee.as_ref() {
                    // Method call
                    let object_type = self.infer_expr(object);

                    if let Type::Class(class_name) = object_type {
                        let method_sig = self.context.get_class(&class_name)
                            .and_then(|c| c.methods.get(name).cloned());

                        if let Some(ref sig) = method_sig {
                            // Check visibility
                            let mut visibility_error = None;
                            if sig.private {
                                if let Some(current) = &self.context.current_class {
                                    if current != &class_name {
                                        visibility_error = Some(format!("Method '{}' on class '{}' is private and cannot be called from class '{}'", name, class_name, current));
                                    }
                                } else {
                                    visibility_error = Some(format!("Method '{}' on class '{}' is private and cannot be called from global scope", name, class_name));
                                }
                            }

                            if let Some(err) = visibility_error {
                                self.context.add_error(err, 0);
                            }

                            self.check_method_call(sig, args, name, &class_name);
                            let mut return_type = sig.return_type.clone().unwrap_or(Type::Unknown);
                            // If calling an async method, return type is Promise<T>
                            if sig.is_async {
                                if let Type::Promise(_) = return_type {
                                    // Already a Promise type
                                } else {
                                    return_type = Type::Promise(Box::new(return_type));
                                }
                            }
                            return_type
                        } else {
                            self.context.add_error(
                                format!("Method '{}' not found on class '{}'", name, class_name),
                                0
                            );
                            Type::Unknown
                        }
                    } else {
                        Type::Unknown
                    }
                } else {
                    Type::Unknown
                }
            }
            Expr::Get { object, name, .. } => {
                let object_type = self.infer_expr(object);

                if let Type::Class(class_name) = object_type {
                    if let Some(class_info) = self.context.get_class(&class_name) {
                        if let Some(field_info) = class_info.fields.get(name) {
                            // Check visibility
                            let mut visibility_error = None;
                            if field_info.private {
                                if let Some(current) = &self.context.current_class {
                                    if current != &class_name {
                                        visibility_error = Some(format!("Field '{}' on class '{}' is private and cannot be accessed from class '{}'", name, class_name, current));
                                    }
                                } else {
                                    visibility_error = Some(format!("Field '{}' on class '{}' is private and cannot be accessed from global scope", name, class_name));
                                }
                            }

                            let type_name = field_info.type_name.clone();

                            if let Some(err) = visibility_error {
                                self.context.add_error(err, 0);
                            }

                            type_name
                        } else {
                            self.context.add_error(
                                format!("Field '{}' not found on class '{}'", name, class_name),
                                0
                            );
                            Type::Unknown
                        }
                    } else {
                        Type::Unknown
                    }
                } else if let Type::Enum(enum_name) = object_type {
                    // Enum variant access - enum variants are integers
                    if let Some(enum_info) = self.context.get_enum(&enum_name) {
                        if let Some(_variant) = enum_info.variants.get(name) {
                            Type::Int  // Enum variants are integers
                        } else {
                            self.context.add_error(
                                format!("Variant '{}' not found on enum '{}'", name, enum_name),
                                0
                            );
                            Type::Unknown
                        }
                    } else {
                        Type::Unknown
                    }
                } else {
                    Type::Unknown
                }
            }
            Expr::Set { object, name, value, span } => {
                let object_type = self.infer_expr(object);
                let value_type = self.infer_expr(value);

                if let Type::Class(class_name) = object_type {
                    let field_info = self.context.get_class(&class_name)
                        .and_then(|c| c.fields.get(name).cloned());

                    if let Some(ref field) = field_info {
                        // Check visibility
                        let mut visibility_error = None;
                        if field.private {
                            if let Some(current) = &self.context.current_class {
                                if current != &class_name {
                                    visibility_error = Some(format!("Field '{}' on class '{}' is private and cannot be modified from class '{}'", name, class_name, current));
                                }
                            } else {
                                visibility_error = Some(format!("Field '{}' on class '{}' is private and cannot be modified from global scope", name, class_name));
                            }
                        }

                        if let Some(err) = visibility_error {
                            self.context.add_error_with_location(err, span.line, span.column, None, None);
                        }

                        if !value_type.is_assignable_to(&field.type_name) {
                            self.context.add_error_with_location(
                                format!(
                                    "Cannot assign {} to field '{}' of type {}",
                                    value_type.to_str(),
                                    name,
                                    field.type_name.to_str()
                                ),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        field.type_name.clone()
                    } else {
                        self.context.add_error_with_location(
                            format!("Field '{}' not found on class '{}'", name, class_name),
                            span.line,
                            span.column,
                            None,
                            None,
                        );
                        Type::Unknown
                    }
                } else {
                    Type::Unknown
                }
            }
            Expr::Interpolated { parts, .. } => {
                for part in parts {
                    if let crate::parser::InterpPart::Expr(e) = part {
                        self.infer_expr(e);
                    }
                }
                Type::Str
            }
            Expr::Range { start, end, .. } => {
                let start_type = self.infer_expr(start);
                let end_type = self.infer_expr(end);
                if start_type != Type::Int && start_type != Type::Unknown {
                    self.context.add_error(format!("Range start must be an integer, got {}", start_type.to_str()), 0);
                }
                if end_type != Type::Int && end_type != Type::Unknown {
                    self.context.add_error(format!("Range end must be an integer, got {}", end_type.to_str()), 0);
                }
                Type::Int
            }
            Expr::Await { expr, .. } => {
                let inner_type = self.infer_expr(expr);
                // Await unwraps Promise<T> to T
                match inner_type {
                    Type::Promise(t) => *t,
                    Type::Unknown => Type::Unknown,
                    _ => {
                        self.context.add_error(
                            format!("Can only await Promise values, got {}", inner_type.to_str()),
                            0
                        );
                        Type::Unknown
                    }
                }
            }
            Expr::Cast { expr, target_type, .. } => {
                // Type check the inner expression
                let inner_type = self.infer_expr(expr);
                
                // Validate cast is reasonable (allow most casts, warn about nonsensical ones)
                match target_type {
                    CastType::Int => {
                        // int() can accept: float, str, bool, int
                        if inner_type != Type::Float && 
                           inner_type != Type::Str && 
                           inner_type != Type::Bool && 
                           inner_type != Type::Int &&
                           inner_type != Type::Unknown {
                            self.context.add_error(
                                format!("Cannot cast {} to int", inner_type.to_str()),
                                0
                            );
                        }
                    }
                    CastType::Float => {
                        // float() can accept: int, str, bool, float
                        if inner_type != Type::Int && 
                           inner_type != Type::Str && 
                           inner_type != Type::Bool &&
                           inner_type != Type::Float &&
                           inner_type != Type::Unknown {
                            self.context.add_error(
                                format!("Cannot cast {} to float", inner_type.to_str()),
                                0
                            );
                        }
                    }
                    CastType::Str => {
                        // str() can accept any type
                    }
                    CastType::Bool => {
                        // bool() can accept: int, float, str, bool
                        if inner_type != Type::Int && 
                           inner_type != Type::Float && 
                           inner_type != Type::Str &&
                           inner_type != Type::Bool &&
                           inner_type != Type::Unknown {
                            self.context.add_error(
                                format!("Cannot cast {} to bool", inner_type.to_str()),
                                0
                            );
                        }
                    }
                }
                
                // Return the target type
                match target_type {
                    CastType::Int => Type::Int,
                    CastType::Float => Type::Float,
                    CastType::Str => Type::Str,
                    CastType::Bool => Type::Bool,
                }
            }
        }
    }

    fn check_function_call(&mut self, func_sig: &FunctionSignature, args: &[Expr], func_name: &str) {
        // Check argument count
        if args.len() != func_sig.params.len() {
            self.context.add_error(
                format!(
                    "Function '{}' expects {} arguments, got {}",
                    func_name,
                    func_sig.params.len(),
                    args.len()
                ),
                0
            );
            return;
        }

        // Check argument types
        for (i, (arg, param)) in args.iter().zip(func_sig.params.iter()).enumerate() {
            let arg_type = self.infer_expr(arg);
            
            if let Some(expected_type) = &param.type_name {
                if !arg_type.is_assignable_to(expected_type) && arg_type != Type::Unknown {
                    self.context.add_error(
                        format!(
                            "Argument {} of function '{}' has wrong type: expected {}, got {}",
                            i + 1,
                            func_name,
                            expected_type.to_str(),
                            arg_type.to_str()
                        ),
                        0
                    );
                }
            }
        }
    }

    fn check_method_call(&mut self, method_sig: &MethodSignature, args: &[Expr], method_name: &str, class_name: &str) {
        // Check argument count (excluding self)
        if args.len() != method_sig.params.len() {
            self.context.add_error(
                format!(
                    "Method '{}' on class '{}' expects {} arguments, got {}",
                    method_name,
                    class_name,
                    method_sig.params.len(),
                    args.len()
                ),
                0
            );
            return;
        }

        // Check argument types
        for (i, (arg, param)) in args.iter().zip(method_sig.params.iter()).enumerate() {
            let arg_type = self.infer_expr(arg);
            
            if let Some(expected_type) = &param.type_name {
                if !arg_type.is_assignable_to(expected_type) && arg_type != Type::Unknown {
                    self.context.add_error(
                        format!(
                            "Argument {} of method '{}' has wrong type: expected {}, got {}",
                            i + 1,
                            method_name,
                            expected_type.to_str(),
                            arg_type.to_str()
                        ),
                        0
                    );
                }
            }
        }
    }
}
