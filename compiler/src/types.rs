use std::collections::HashMap;
use crate::parser::{ClassDef, Method, Expr, Literal, Stmt};

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
    Optional(Box<Type>),
    Null,
    Unknown,
}

impl Type {
    pub fn from_str(s: &str) -> Self {
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
            Type::Optional(t) => format!("{}?", t.to_str()),
            Type::Null => "null".to_string(),
            Type::Unknown => "unknown".to_string(),
        }
    }

    pub fn is_assignable_to(&self, other: &Type) -> bool {
        match (self, other) {
            (Type::Null, Type::Optional(_)) => true,
            (Type::Optional(inner), other) => inner.is_assignable_to(other),
            (a, b) if a == b => true,
            (Type::Int, Type::Float) => true, // Allow int to float coercion
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
    pub current_class: Option<String>,
    pub current_method_return: Option<Type>,
    pub imports: Vec<String>,
    pub errors: Vec<TypeError>,
}

#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub line: usize,
}

impl TypeContext {
    pub fn new() -> Self {
        let mut ctx = Self {
            classes: HashMap::new(),
            functions: HashMap::new(),
            variables: HashMap::new(),
            current_class: None,
            current_method_return: None,
            imports: Vec::new(),
            errors: Vec::new(),
        };
        
        // Register native classes
        ctx.register_native_classes();
        
        ctx
    }

    fn register_native_classes(&mut self) {
        // Register std.io module functions
        let io_print = FunctionSignature {
            name: "print".to_string(),
            params: vec![ParamSignature {
                name: "text".to_string(),
                type_name: Some(Type::Str),
            }],
            return_type: None,
            return_optional: false,
            is_method: false,
        };

        let io_println = FunctionSignature {
            name: "println".to_string(),
            params: vec![ParamSignature {
                name: "line".to_string(),
                type_name: Some(Type::Str),
            }],
            return_type: None,
            return_optional: false,
            is_method: false,
        };

        self.functions.insert("print".to_string(), io_print);
        self.functions.insert("println".to_string(), io_println);
        
        // Mark std.io as imported by default for native functions
        self.imports.push("std.io".to_string());
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

    pub fn get_method(&self, class_name: &str, method_name: &str) -> Option<&MethodSignature> {
        self.classes.get(class_name).and_then(|c| c.methods.get(method_name))
    }

    pub fn add_error(&mut self, message: String, line: usize) {
        self.errors.push(TypeError { message, line });
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

    pub fn check(&mut self, statements: &[Stmt]) -> Result<&TypeContext, &[TypeError]> {
        // First pass: collect all class and function definitions
        self.collect_definitions(statements);

        // Second pass: type check all statements
        for stmt in statements {
            self.check_stmt(stmt);
        }

        if self.context.has_errors() {
            Err(self.context.get_errors())
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

    fn collect_definitions(&mut self, statements: &[Stmt]) {
        for stmt in statements {
            match stmt {
                Stmt::Class(class) => {
                    self.context.add_class(class);
                }
                Stmt::Function(func) => {
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
                    });
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
            Stmt::Function(func) => {
                self.check_function(func);
            }
            Stmt::Let { name, expr } => {
                let expr_type = self.infer_expr(expr);
                self.context.add_variable(name, expr_type);
            }
            Stmt::Assign { name, expr } => {
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
                } else {
                    // Variable not declared with let, create it
                    self.context.add_variable(name, expr_type);
                }
            }
            Stmt::Return(expr) => {
                if let Some(expected_return) = &self.context.current_method_return.clone() {
                    if let Some(e) = expr {
                        let expr_type = self.infer_expr(e);
                        if !expr_type.is_assignable_to(expected_return) {
                            self.context.add_error(
                                format!(
                                    "Return type mismatch: expected {}, got {}",
                                    expected_return.to_str(),
                                    expr_type.to_str()
                                ),
                                0
                            );
                        }
                    } else if !matches!(expected_return, Type::Null | Type::Unknown) {
                        self.context.add_error(
                            format!(
                                "Expected return value of type {}, but no value returned",
                                expected_return.to_str()
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
        
        // Handle optional return types
        let return_type = func.return_type.as_ref().map(|t| {
            let ty = Type::from_str(t);
            if func.return_optional {
                Type::Optional(Box::new(ty))
            } else {
                ty
            }
        });
        
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
    }

    fn check_method(&mut self, method: &Method, class_name: &str) {
        let old_return = self.context.current_method_return.clone();
        
        // Handle optional return types
        let return_type = method.return_type.as_ref().map(|t| {
            let ty = Type::from_str(t);
            if method.return_optional {
                Type::Optional(Box::new(ty))
            } else {
                ty
            }
        });
        
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
            Expr::Variable(name) => {
                if let Some(var_info) = self.context.get_variable(name) {
                    var_info.type_name.clone()
                } else {
                    Type::Unknown
                }
            }
            Expr::Binary { left, op, right } => {
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
                }
            }
            Expr::Unary { op, expr } => {
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
                }
            }
            Expr::Call { callee, args } => {
                if let Expr::Variable(func_name) = callee.as_ref() {
                    // Check if it's a function call
                    let func_sig = self.context.get_function(func_name).cloned();
                    if let Some(ref sig) = func_sig {
                        self.check_function_call(sig, args, func_name);
                        sig.return_type.clone().unwrap_or(Type::Unknown)
                    } else {
                        Type::Unknown
                    }
                } else if let Expr::Get { object, name } = callee.as_ref() {
                    // Method call
                    let object_type = self.infer_expr(object);

                    if let Type::Class(class_name) = object_type {
                        let method_sig = self.context.get_class(&class_name)
                            .and_then(|c| c.methods.get(name).cloned());
                        
                        if let Some(ref sig) = method_sig {
                            self.check_method_call(sig, args, name, &class_name);
                            sig.return_type.clone().unwrap_or(Type::Unknown)
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
            Expr::Get { object, name } => {
                let object_type = self.infer_expr(object);
                
                if let Type::Class(class_name) = object_type {
                    if let Some(class_info) = self.context.get_class(&class_name) {
                        if let Some(field_info) = class_info.fields.get(name) {
                            field_info.type_name.clone()
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
                } else {
                    Type::Unknown
                }
            }
            Expr::Set { object, name, value } => {
                let object_type = self.infer_expr(object);
                let value_type = self.infer_expr(value);

                if let Type::Class(class_name) = object_type {
                    let field_info = self.context.get_class(&class_name)
                        .and_then(|c| c.fields.get(name).cloned());
                    
                    if let Some(ref field) = field_info {
                        if !value_type.is_assignable_to(&field.type_name) {
                            self.context.add_error(
                                format!(
                                    "Cannot assign {} to field '{}' of type {}",
                                    value_type.to_str(),
                                    name,
                                    field.type_name.to_str()
                                ),
                                0
                            );
                        }
                        field.type_name.clone()
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
            }
            Expr::Interpolated { parts } => {
                for part in parts {
                    if let crate::parser::InterpPart::Expr(e) = part {
                        self.infer_expr(e);
                    }
                }
                Type::Str
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
