use std::collections::HashMap;
use crate::parser::{ClassDef, Method, Expr, Literal, Stmt, CastType};

fn is_numeric_type(ty: &Type) -> bool {
    ty.is_numeric()
}

fn is_integer_type(ty: &Type) -> bool {
    matches!(ty, Type::Int | Type::Int8 | Type::UInt8 | Type::Int16 | Type::UInt16 |
             Type::Int32 | Type::UInt32 | Type::Int64 | Type::UInt64)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    Str,
    Bool,
    Int8,
    UInt8,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Int64,
    UInt64,
    Float32,
    Float64,
    Class(String),
    Enum(String),
    Optional(Box<Type>),
    Array(Box<Type>),
    Null,
    Any,
    Unknown,
    // Generics support
    TypeParameter(String),                    // T (type parameter)
    GenericInstance(String, Vec<Type>),       // ClassName<T1, T2, ...>
    // Type alias support
    TypeAlias(String, Vec<Type>),             // Alias name with optional type args
    // Function type for lambdas
    Function(Vec<Type>, Box<Type>),           // (param_types, return_type)
    // Self type for methods (resolved to actual class type during type checking)
    SelfType,
}

impl Type {
    pub fn from_str(s: &str) -> Self {
        // Handle array type suffix []
        if s.ends_with("[]") {
            let inner = s.trim_end_matches("[]");
            return Type::Array(Box::new(Type::from_str(inner)));
        }
        // Handle optional type suffix
        if s.ends_with('?') {
            let inner = s.trim_end_matches('?');
            return Type::Optional(Box::new(Type::from_str(inner)));
        }

        // Handle function types: (param_types) -> return_type  or  (param_types) for void return
        if s.starts_with('(') {
            // First find the closing paren
            if let Some(paren_end) = s.find(')') {
                let params_str = &s[1..paren_end];
                let rest = &s[paren_end + 1..];
                
                let param_types: Vec<Type> = if params_str.trim().is_empty() {
                    Vec::new()
                } else {
                    params_str.split(',')
                        .map(|p| Type::from_str(p.trim()))
                        .collect()
                };

                // Check if there's a return type
                let rest_trimmed = rest.trim();
                if rest_trimmed.starts_with("-> ") || rest_trimmed.starts_with("->") {
                    let return_str = rest_trimmed.trim_start_matches("->").trim();
                    let return_type = Box::new(Type::from_str(return_str.trim()));
                    return Type::Function(param_types, return_type);
                } else {
                    // No return type - function returns nothing (void)
                    return Type::Function(param_types, Box::new(Type::Null));
                }
            }
        }

        // Handle generic types like ClassName<T1, T2>
        if let Some(angle_start) = s.find('<') {
            if s.ends_with('>') {
                let class_name = &s[..angle_start];
                let args_str = &s[angle_start + 1..s.len() - 1];
                let args = Self::parse_generic_args(args_str);
                return Type::GenericInstance(class_name.to_string(), args);
            }
        }

        match s {
            "int" => Type::Int,
            "float" => Type::Float,
            "str" => Type::Str,
            "bool" => Type::Bool,
            "int8" => Type::Int8,
            "uint8" => Type::UInt8,
            "int16" => Type::Int16,
            "uint16" => Type::UInt16,
            "int32" => Type::Int32,
            "uint32" => Type::UInt32,
            "int64" => Type::Int64,
            "uint64" => Type::UInt64,
            "float32" => Type::Float32,
            "float64" => Type::Float64,
            "any" => Type::Any,
            "self" => Type::SelfType,
            // Recognize type parameters: single uppercase letter (T, U, V, etc.) or uppercase followed by single digit (T1, U2)
            _ if s.len() <= 2 && s.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) && s.chars().skip(1).all(|c| c.is_ascii_digit()) => {
                Type::TypeParameter(s.to_string())
            }
            _ => Type::Class(s.to_string()),
        }
    }

    fn parse_generic_args(args_str: &str) -> Vec<Type> {
        let mut args = Vec::new();
        let mut depth = 0;
        let mut current = String::new();

        for ch in args_str.chars() {
            match ch {
                '<' => {
                    depth += 1;
                    current.push(ch);
                }
                '>' => {
                    depth -= 1;
                    current.push(ch);
                }
                ',' if depth == 0 => {
                    let arg = current.trim().to_string();
                    if !arg.is_empty() {
                        args.push(Type::from_str(&arg));
                    }
                    current.clear();
                }
                _ => {
                    current.push(ch);
                }
            }
        }

        let arg = current.trim().to_string();
        if !arg.is_empty() {
            args.push(Type::from_str(&arg));
        }

        args
    }

    pub fn to_str(&self) -> String {
        match self {
            Type::Int => "int".to_string(),
            Type::Float => "float".to_string(),
            Type::Str => "str".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Int8 => "int8".to_string(),
            Type::UInt8 => "uint8".to_string(),
            Type::Int16 => "int16".to_string(),
            Type::UInt16 => "uint16".to_string(),
            Type::Int32 => "int32".to_string(),
            Type::UInt32 => "uint32".to_string(),
            Type::Int64 => "int64".to_string(),
            Type::UInt64 => "uint64".to_string(),
            Type::Float32 => "float32".to_string(),
            Type::Float64 => "float64".to_string(),
            Type::Class(name) => name.clone(),
            Type::Enum(name) => name.clone(),
            Type::Optional(t) => format!("{}?", t.to_str()),
            Type::Array(t) => format!("{}[]", t.to_str()),
            Type::Null => "null".to_string(),
            Type::Unknown => "unknown".to_string(),
            Type::TypeParameter(name) => name.clone(),
            Type::GenericInstance(name, args) => {
                let args_str: Vec<String> = args.iter().map(|a| a.to_str()).collect();
                format!("{}<{}>", name, args_str.join(", "))
            }
            Type::TypeAlias(name, args) => {
                if args.is_empty() {
                    name.clone()
                } else {
                    let args_str: Vec<String> = args.iter().map(|a| a.to_str()).collect();
                    format!("{}<{}>", name, args_str.join(", "))
                }
            }
            Type::Function(params, return_type) => {
                let params_str: Vec<String> = params.iter().map(|p| p.to_str()).collect();
                if matches!(return_type.as_ref(), Type::Null) {
                    // No return type (void) - just show parameters
                    format!("({})", params_str.join(", "))
                } else {
                    format!("({}) -> {}", params_str.join(", "), return_type.to_str())
                }
            }
            Type::SelfType => "self".to_string(),
            Type::Any => "any".to_string(),
        }
    }

    /// Convert type to string for mangling purposes (flattens generic arguments)
    /// Format: ClassName<T,B> becomes ClassName(T,B) for use in function mangling
    /// Type parameters (T, U, etc.) and Unknown are represented as * for generic functions
    pub fn to_mangle_str(&self) -> String {
        match self {
            Type::Int => "int".to_string(),
            Type::Float => "float".to_string(),
            Type::Str => "str".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Int8 => "int8".to_string(),
            Type::UInt8 => "uint8".to_string(),
            Type::Int16 => "int16".to_string(),
            Type::UInt16 => "uint16".to_string(),
            Type::Int32 => "int32".to_string(),
            Type::UInt32 => "uint32".to_string(),
            Type::Int64 => "int64".to_string(),
            Type::UInt64 => "uint64".to_string(),
            Type::Float32 => "float32".to_string(),
            Type::Float64 => "float64".to_string(),
            Type::Class(name) => name.clone(),
            Type::Enum(name) => name.clone(),
            Type::Optional(t) => format!("{}?", t.to_mangle_str()),
            Type::Array(t) => format!("{}[]", t.to_mangle_str()),
            Type::Null => "null".to_string(),
            Type::Unknown => "*".to_string(),  // Unknown type -> *
            Type::TypeParameter(_) => "*".to_string(),  // Generic type parameter -> *
            Type::GenericInstance(name, args) => {
                let args_str: Vec<String> = args.iter().map(|a| a.to_mangle_str()).collect();
                format!("{}({})", name, args_str.join(","))
            }
            Type::TypeAlias(name, args) => {
                if args.is_empty() {
                    name.clone()
                } else {
                    let args_str: Vec<String> = args.iter().map(|a| a.to_mangle_str()).collect();
                    format!("{}({})", name, args_str.join(","))
                }
            }
            Type::Function(params, return_type) => {
                let params_str: Vec<String> = params.iter().map(|p| p.to_mangle_str()).collect();
                if matches!(return_type.as_ref(), Type::Null) {
                    // No return type (void) - just show parameters
                    format!("({})", params_str.join(","))
                } else {
                    format!("({}) -> {}", params_str.join(","), return_type.to_mangle_str())
                }
            }
            Type::SelfType => "self".to_string(),
            Type::Any => "any".to_string(),
        }
    }

    pub fn is_assignable_to(&self, other: &Type) -> bool {
        match (self, other) {
            (_, Type::Any) => true,
            (Type::Any, _) => true,
            (Type::Null, Type::Optional(_)) => true,
            (inner, Type::Optional(target)) => inner.is_assignable_to(target),
            (Type::Optional(inner), other) => inner.is_assignable_to(other),
            (Type::Array(a), Type::Array(b)) => a.is_assignable_to(b),
            // Function type compatibility
            (Type::Function(self_params, self_return), Type::Function(other_params, other_return)) => {
                if self_params.len() != other_params.len() {
                    return false;
                }
                // Parameters must match exactly
                let params_match = self_params.iter().zip(other_params.iter())
                    .all(|(s, o)| s == o || s == &Type::Unknown || o == &Type::Unknown);
                // Return type must be compatible
                let return_match = self_return.is_assignable_to(other_return);
                params_match && return_match
            }
            // Generic instance matching
            (Type::GenericInstance(a_name, a_args), Type::GenericInstance(b_name, b_args)) => {
                if a_name != b_name || a_args.len() != b_args.len() {
                    return false;
                }
                a_args.iter().zip(b_args.iter()).all(|(a, b)| a.is_assignable_to(b))
            }
            // Type alias resolution
            (Type::TypeAlias(_, _), _) => {
                // For now, type aliases are transparent - they should be resolved before this check
                false
            }
            (_, Type::TypeAlias(_, _)) => false,
            // Type parameters - very permissive for now (will be refined in type checker)
            (Type::TypeParameter(_), _) => true,
            (_, Type::TypeParameter(_)) => true,
            // Self type - only matches with itself (will be resolved to actual class type during type checking)
            (Type::SelfType, Type::SelfType) => true,
            (a, b) if a == b => true,
            // Numeric compatibility
            (Type::Int, Type::Float) => true,
            (Type::Int, Type::Int64) => true,
            (Type::Int64, Type::Int) => true,
            (Type::Float, Type::Float64) => true,
            (Type::Float64, Type::Float) => true,
            // All numeric types are somewhat compatible for now as requested for FFI/ByteBuffer
            (a, b) if a.is_numeric() && b.is_numeric() => true,
            (_, Type::Unknown) => true,
            (Type::Unknown, _) => true,
            _ => false,
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Int | Type::Float | Type::Int8 | Type::UInt8 | Type::Int16 | Type::UInt16 | Type::Int32 | Type::UInt32 | Type::Int64 | Type::UInt64 | Type::Float32 | Type::Float64)
    }

    pub fn is_pod(&self) -> bool {
        match self {
            Type::Int | Type::Float | Type::Str | Type::Bool |
            Type::Int8 | Type::UInt8 | Type::Int16 | Type::UInt16 |
            Type::Int32 | Type::UInt32 | Type::Int64 | Type::UInt64 |
            Type::Float32 | Type::Float64 | Type::Null => true,
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
    pub is_native: bool,
    pub private: bool,
    pub type_params: Vec<String>,
    /// Mangled name for VM lookup (includes type information for overloading)
    pub mangled_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParamSignature {
    pub name: String,
    pub type_name: Option<Type>,
    pub default: bool,  // true if this parameter has a default value
}

/// Universal mangle function for creating unique identifiers
/// Format: [package.]class.name(args) or package.name(args) depending on context
/// 
/// # Arguments
/// * `package` - Optional package/module path (e.g., "std.io")
/// * `class` - Optional class name for methods (e.g., "MyClass")
/// * `name` - The base function/method name (e.g., "println" or "Test_constructor()")
/// * `args` - Parameter types for mangling (e.g., [Type::Str, Type::Int])
///
/// # Examples
/// - mangle(Some("std.io"), None, "println", &[Type::Str]) -> "std.io.println(str)"
/// - mangle(None, Some("MyClass"), "method", &[Type::Int]) -> "MyClass.method(int)"
/// - mangle(Some("std"), Some("io"), "print", &[Type::Str]) -> "std.io.print(str)"
/// - mangle(None, None, "main", &[]) -> "main()"
/// - mangle(None, Some("Test"), "constructor", &[]) -> "Test.constructor()"
/// - mangle(None, Some("Test"), "constructor", &[Type::Str]) -> "Test.constructor(str)"
pub fn mangle(package: Option<&str>, class: Option<&str>, name: &str, args: &[Type]) -> String {
    let mut mangled = String::new();

    // Add package prefix if present
    if let Some(pkg) = package {
        mangled.push_str(pkg);
        mangled.push('.');
    }

    // Add class prefix if present
    if let Some(cls) = class {
        mangled.push_str(cls);
        mangled.push('.');
    }

    // Handle constructor names specially: convert "constructor" or "constructor()" to ".constructor()" format
    if name == "constructor" || name.starts_with("constructor(") {
        // Extract args from "constructor(args)" if present, otherwise use provided args
        let constructor_args = if name.starts_with("constructor(") && name.ends_with(')') {
            // Parse args from the name itself
            let args_str = &name[12..name.len()-1]; // Extract content between "constructor(" and ")"
            if args_str.is_empty() {
                Vec::new()
            } else {
                args_str.split(',').map(|s| Type::from_str(s.trim())).collect()
            }
        } else {
            args.to_vec()
        };
        
        mangled.push_str("constructor");
        mangled.push('(');
        for (i, ty) in constructor_args.iter().enumerate() {
            if i > 0 {
                mangled.push(',');
            }
            mangled.push_str(&ty.to_mangle_str());
        }
        mangled.push(')');
        return mangled;
    }

    // Check if name already has a signature (ends with ))
    if name.ends_with(')') {
        // Name already has signature (e.g., "method(int)" or "_constructor()")
        // Convert old _constructor() format to new .constructor() format
        if name.contains("_constructor()") {
            let base_name = name.replace("_constructor()", "");
            return format!("{}{}.constructor()", mangled, base_name);
        }
        // Use as-is without additional mangling
        return format!("{}{}", mangled, name);
    }

    // Add base name
    mangled.push_str(name);

    // Add argument signature
    mangled.push('(');
    for (i, ty) in args.iter().enumerate() {
        if i > 0 {
            mangled.push(',');
        }
        mangled.push_str(&ty.to_mangle_str());
    }
    mangled.push(')');

    mangled
}

/// Generate a mangled name for a function based on its parameter types
/// This enables function overloading by creating unique names for each signature
/// Format: <name>(<type1>,<type2>,...) - simple and readable mangling
/// 
/// Deprecated: Use the universal `mangle(package, class, name, args)` function instead
pub fn mangle_function_name(name: &str, param_types: &[Type]) -> String {
    mangle(None, None, name, param_types)
}

#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub name: String,
    pub type_params: Vec<String>,
    pub fields: HashMap<String, FieldInfo>,
    /// Methods stored by mangled name (e.g., "foo@i_s" for foo(int, str))
    pub methods: HashMap<String, MethodSignature>,
    /// Map from base method name to list of mangled names (for overload resolution)
    pub method_overloads: HashMap<String, Vec<String>>,
    pub parent_interfaces: Vec<String>,
    pub vtable: Vec<String>,  // Ordered list of virtual method names
    pub is_interface: bool,
    pub is_native: bool,
    pub private: bool,
}

#[derive(Debug, Clone)]
pub struct InterfaceInfo {
    pub name: String,
    pub type_params: Vec<String>,
    pub parent_interfaces: Vec<String>,
    /// Methods stored by mangled name
    pub methods: HashMap<String, MethodSignature>,
    /// Map from base method name to list of mangled names (for overload resolution)
    pub method_overloads: HashMap<String, Vec<String>>,
    pub vtable: Vec<String>,  // Ordered list of method names for vtable
    pub private: bool,
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub type_name: Type,
    pub private: bool,
    pub is_static: bool,
}

#[derive(Debug, Clone)]
pub struct MethodSignature {
    pub name: String,
    pub params: Vec<ParamSignature>,
    pub return_type: Option<Type>,
    pub return_optional: bool,
    pub private: bool,
    pub is_native: bool,
    pub is_static: bool,
    pub type_params: Vec<String>,
    pub mangled_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub name: String,
    pub variants: HashMap<String, EnumVariantInfo>,
    pub private: bool,
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
    pub private: bool,
}

#[derive(Debug, Clone)]
pub struct TypeAliasInfo {
    pub name: String,
    pub type_params: Vec<String>,
    pub aliased_type: Type,
    pub private: bool,
}

/// Represents what a single import brings into scope
#[derive(Debug, Clone)]
pub struct ImportEntry {
    /// The full module path that was imported (e.g., "std.io")
    pub module_path: String,
    /// The alias or name this import brings into scope
    /// - For "import std.io": Some("io")
    /// - For "import std.io as myio": Some("myio")
    /// - For "import std.io.println": Some("println")
    /// - For "import std": None (module import, access via std.xxx)
    pub alias: Option<String>,
    /// The kind of import
    pub kind: crate::parser::ImportKind,
    /// All members brought into scope (for wildcard imports)
    pub members: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TypeContext {
    pub classes: HashMap<String, ClassInfo>,
    pub interfaces: HashMap<String, InterfaceInfo>,
    /// Functions stored by mangled name (e.g., "foo@i_s" for foo(int, str))
    pub functions: HashMap<String, FunctionSignature>,
    /// Map from base function name to list of mangled names (for overload resolution)
    pub function_overloads: HashMap<String, Vec<String>>,
    pub variables: HashMap<String, VariableInfo>,
    pub enums: HashMap<String, EnumInfo>,
    pub type_aliases: HashMap<String, TypeAliasInfo>,
    pub current_class: Option<String>,
    pub current_module: Option<String>,
    pub current_method_return: Option<Type>,
    pub current_method_params: Vec<String>,
    /// List of imports with their scope information
    pub imports: Vec<ImportEntry>,
    /// Raw import paths for backward compatibility
    pub import_paths: Vec<String>,
    pub errors: Vec<TypeError>,
    // Type annotations for expressions (line -> column -> type)
    // Used to store inferred types for object literals
    pub expr_types: HashMap<(usize, usize), String>,
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
            interfaces: HashMap::new(),
            functions: HashMap::new(),
            function_overloads: HashMap::new(),
            variables: HashMap::new(),
            enums: HashMap::new(),
            type_aliases: HashMap::new(),
            current_class: None,
            current_module: None,
            current_method_return: None,
            current_method_params: Vec::new(),
            imports: Vec::new(),
            import_paths: Vec::new(),
            errors: Vec::new(),
            expr_types: HashMap::new(),
        };

        // Register built-in global variables
        ctx.variables.insert("ARGV".to_string(), crate::types::VariableInfo {
            name: "ARGV".to_string(),
            type_name: Type::Array(Box::new(Type::Str)),
            private: false,
        });

        // Register native classes
        ctx.register_native_classes();

        ctx
    }

    fn register_native_classes(&mut self) {
        // Register std.io module functions (only when std.io is imported)
        // These are registered in ModuleResolver::register_native_functions()

        // JSON
        let json_stringify = FunctionSignature {
            name: "std.json.stringify".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::Str),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("std.json.stringify", json_stringify);

        let json_parse = FunctionSignature {
            name: "std.json.parse".to_string(),
            params: vec![ParamSignature {
                name: "json".to_string(),
                type_name: Some(Type::Str),
                default: false,
            }],
            return_type: Some(Type::Unknown),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("std.json.parse", json_parse);

        self.import_paths.push("std.json".to_string());

        // Reflection
        let reflect_typeof = FunctionSignature {
            name: "std.reflect.type_of".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::Str),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("std.reflect.type_of", reflect_typeof);

        let reflect_class_name = FunctionSignature {
            name: "std.reflect.class_name".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::Str),
            return_optional: true,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("std.reflect.class_name", reflect_class_name);

        let reflect_fields = FunctionSignature {
            name: "std.reflect.fields".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::Unknown),
            return_optional: true,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("std.reflect.fields", reflect_fields);

        self.import_paths.push("std.reflect".to_string());

        // std.io functions (print, println) are registered only when std.io is explicitly imported
        // See ModuleResolver::register_std_io_import() in resolver.rs

        // Built-in types methods
        // str methods
        let mut str_methods = HashMap::new();
        str_methods.insert("length()".to_string(), MethodSignature {
            name: "length".to_string(),
            params: vec![],
            return_type: Some(Type::Int),
            return_optional: false,
            private: false,
            is_native: true,
            is_static: false,
            type_params: Vec::new(),
            mangled_name: Some("length()".to_string()),
        });
        str_methods.insert("trim()".to_string(), MethodSignature {
            name: "trim".to_string(),
            params: vec![],
            return_type: Some(Type::Str),
            return_optional: false,
            private: false,
            is_native: true,
            is_static: false,
            type_params: Vec::new(),
            mangled_name: Some("trim()".to_string()),
        });
        str_methods.insert("split(str)".to_string(), MethodSignature {
            name: "split".to_string(),
            params: vec![ParamSignature { name: "delimiter".to_string(), type_name: Some(Type::Str), default: false }],
            return_type: Some(Type::Array(Box::new(Type::Str))),
            return_optional: false,
            private: false,
            is_native: true,
            is_static: false,
            type_params: Vec::new(),
            mangled_name: Some("split(str)".to_string()),
        });
        str_methods.insert("toInt()".to_string(), MethodSignature {
            name: "toInt".to_string(),
            params: vec![],
            return_type: Some(Type::Int),
            return_optional: false,
            private: false,
            is_native: true,
            is_static: false,
            type_params: Vec::new(),
            mangled_name: Some("toInt()".to_string()),
        });
        str_methods.insert("toFloat()".to_string(), MethodSignature {
            name: "toFloat".to_string(),
            params: vec![],
            return_type: Some(Type::Float),
            return_optional: false,
            private: false,
            is_native: true,
            is_static: false,
            type_params: Vec::new(),
            mangled_name: Some("toFloat()".to_string()),
        });
        str_methods.insert("contains(str)".to_string(), MethodSignature {
            name: "contains".to_string(),
            params: vec![ParamSignature { name: "substr".to_string(), type_name: Some(Type::Str), default: false }],
            return_type: Some(Type::Bool),
            return_optional: false,
            private: false,
            is_native: true,
            is_static: false,
            type_params: Vec::new(),
            mangled_name: Some("contains(str)".to_string()),
        });
        str_methods.insert("startsWith(str)".to_string(), MethodSignature {
            name: "startsWith".to_string(),
            params: vec![ParamSignature { name: "prefix".to_string(), type_name: Some(Type::Str), default: false }],
            return_type: Some(Type::Bool),
            return_optional: false,
            private: false,
            is_native: true,
            is_static: false,
            type_params: Vec::new(),
            mangled_name: Some("startsWith(str)".to_string()),
        });
        str_methods.insert("endsWith(str)".to_string(), MethodSignature {
            name: "endsWith".to_string(),
            params: vec![ParamSignature { name: "suffix".to_string(), type_name: Some(Type::Str), default: false }],
            return_type: Some(Type::Bool),
            return_optional: false,
            private: false,
            is_native: true,
            is_static: false,
            type_params: Vec::new(),
            mangled_name: Some("endsWith(str)".to_string()),
        });
        str_methods.insert("substring(int,int)".to_string(), MethodSignature {
            name: "substring".to_string(),
            params: vec![
                ParamSignature { name: "start".to_string(), type_name: Some(Type::Int), default: false },
                ParamSignature { name: "end".to_string(), type_name: Some(Type::Int), default: false },
            ],
            return_type: Some(Type::Str),
            return_optional: false,
            private: false,
            is_native: true,
            is_static: false,
            type_params: Vec::new(),
            mangled_name: Some("substring(int,int)".to_string()),
        });
        let mut str_method_overloads = HashMap::new();
        str_method_overloads.insert("length".to_string(), vec!["length()".to_string()]);
        str_method_overloads.insert("trim".to_string(), vec!["trim()".to_string()]);
        str_method_overloads.insert("split".to_string(), vec!["split(str)".to_string()]);
        str_method_overloads.insert("toInt".to_string(), vec!["toInt()".to_string()]);
        str_method_overloads.insert("toFloat".to_string(), vec!["toFloat()".to_string()]);
        str_method_overloads.insert("contains".to_string(), vec!["contains(str)".to_string()]);
        str_method_overloads.insert("startsWith".to_string(), vec!["startsWith(str)".to_string()]);
        str_method_overloads.insert("endsWith".to_string(), vec!["endsWith(str)".to_string()]);
        str_method_overloads.insert("substring".to_string(), vec!["substring(int,int)".to_string()]);
        self.classes.insert("str".to_string(), ClassInfo {
            name: "str".to_string(),
            fields: HashMap::new(),
            methods: str_methods,
            method_overloads: str_method_overloads,
            vtable: vec!["length".to_string(), "trim".to_string(), "split".to_string(), "toInt".to_string(), "toFloat".to_string(), "contains".to_string(), "startsWith".to_string(), "endsWith".to_string(), "substring".to_string()],
            is_native: true,
            is_interface: false,
            private: false,
            parent_interfaces: vec![],
            type_params: vec![],
        });

        // Array methods
        let mut array_methods = HashMap::new();
        array_methods.insert("length()".to_string(), MethodSignature {
            name: "length".to_string(),
            params: vec![],
            return_type: Some(Type::Int),
            return_optional: false,
            private: false,
            is_native: true,
            is_static: false,
            type_params: Vec::new(),
            mangled_name: Some("length()".to_string()),
        });
        array_methods.insert("add(Unknown)".to_string(), MethodSignature {
            name: "add".to_string(),
            params: vec![ParamSignature { name: "element".to_string(), type_name: Some(Type::Unknown), default: false }],
            return_type: None,
            return_optional: false,
            private: false,
            is_native: true,
            is_static: false,
            type_params: Vec::new(),
            mangled_name: Some("add(Unknown)".to_string()),
        });
        let mut array_method_overloads = HashMap::new();
        array_method_overloads.insert("length".to_string(), vec!["length()".to_string()]);
        array_method_overloads.insert("add".to_string(), vec!["add(Unknown)".to_string()]);
        self.classes.insert("Array".to_string(), ClassInfo {
            name: "Array".to_string(),
            fields: HashMap::new(),
            methods: array_methods,
            method_overloads: array_method_overloads,
            vtable: vec!["length".to_string(), "add".to_string()],
            is_native: true,
            is_interface: false,
            private: false,
            parent_interfaces: vec![],
            type_params: vec!["T".to_string()],
        });

        // Register global str() function
        let str_fn = FunctionSignature {
            name: "str".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::Str),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("str", str_fn);

        // Register int() function
        let int_fn = FunctionSignature {
            name: "int".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::Int),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("int", int_fn);

        // Register float() function
        let float_fn = FunctionSignature {
            name: "float".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::Float),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("float", float_fn);

        // Register bool() function
        let bool_fn = FunctionSignature {
            name: "bool".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::Bool),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("bool", bool_fn);

        // Register int8() function
        let int8_fn = FunctionSignature {
            name: "int8".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::Int8),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("int8", int8_fn);

        // Register uint8() function
        let uint8_fn = FunctionSignature {
            name: "uint8".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::UInt8),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("uint8", uint8_fn);

        // Register int16() function
        let int16_fn = FunctionSignature {
            name: "int16".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::Int16),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("int16", int16_fn);

        // Register uint16() function
        let uint16_fn = FunctionSignature {
            name: "uint16".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::UInt16),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("uint16", uint16_fn);

        // Register int32() function
        let int32_fn = FunctionSignature {
            name: "int32".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::Int32),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("int32", int32_fn);

        // Register uint32() function
        let uint32_fn = FunctionSignature {
            name: "uint32".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::UInt32),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("uint32", uint32_fn);

        // Register int64() function
        let int64_fn = FunctionSignature {
            name: "int64".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::Int64),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("int64", int64_fn);

        // Register uint64() function
        let uint64_fn = FunctionSignature {
            name: "uint64".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::UInt64),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("uint64", uint64_fn);

        // Register float32() function
        let float32_fn = FunctionSignature {
            name: "float32".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::Float32),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("float32", float32_fn);

        // Register float64() function
        let float64_fn = FunctionSignature {
            name: "float64".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
                default: false,
            }],
            return_type: Some(Type::Float64),
            return_optional: false,
            is_method: false,
            is_native: true,
            private: false,
            type_params: Vec::new(),
            mangled_name: None,
        };
        self.add_function("float64", float64_fn);
    }

    pub fn add_class(&mut self, class: &ClassDef) {
        let mut fields = HashMap::new();
        for field in &class.fields {
            fields.insert(field.name.clone(), FieldInfo {
                name: field.name.clone(),
                type_name: Type::from_str(&field.type_name),
                private: field.private,
                is_static: field.is_static,
            });
        }

        let mut methods = HashMap::new();
        let mut method_overloads: HashMap<String, Vec<String>> = HashMap::new();
        let mut has_constructor = false;

        for method in &class.methods {
            if method.name == "constructor" {
                has_constructor = true;
            }
            let params: Vec<ParamSignature> = method.params.iter().map(|p| ParamSignature {
                name: p.name.clone(),
                type_name: p.type_name.as_ref().map(|t| Type::from_str(t)),
                default: false,
            }).collect();

            // Generate mangled name for the method
            let param_types: Vec<Type> = params.iter()
                .filter_map(|p| p.type_name.clone())
                .collect();
            let mangled = mangle_function_name(&method.name, &param_types);

            let overloads = method_overloads.entry(method.name.clone()).or_insert_with(Vec::new);
            overloads.push(mangled.clone());

            methods.insert(mangled.clone(), MethodSignature {
                name: method.name.clone(),
                params,
                return_type: method.return_type.as_ref().map(|t| Type::from_str(t)),
                return_optional: method.return_optional,
                private: method.private,
                is_native: method.is_native,
                is_static: method.is_static,
                type_params: method.type_params.clone(),
                mangled_name: Some(mangled.clone()),
            });
        }

        // Auto-generate constructors if no custom constructor is defined
        if !has_constructor {
            // Empty constructor() - fields can be initialized with defaults or remain uninitialized
            let empty_ctor_mangled = mangle_function_name("constructor", &[]);
            let overloads = method_overloads.entry("constructor".to_string()).or_insert_with(Vec::new);
            overloads.push(empty_ctor_mangled.clone());

            methods.insert(empty_ctor_mangled.clone(), MethodSignature {
                name: "constructor".to_string(),
                params: Vec::new(),
                return_type: None,
                return_optional: false,
                private: false,
                is_native: false,
                is_static: false,
                type_params: Vec::new(),
                mangled_name: Some(empty_ctor_mangled),
            });

            // Constructor with all fields as parameters
            if !class.fields.is_empty() {
                let field_params: Vec<ParamSignature> = class.fields.iter().map(|field| ParamSignature {
                    name: field.name.clone(),
                    type_name: Some(Type::from_str(&field.type_name)),
                    default: false,
                }).collect();
                let field_param_types: Vec<Type> = field_params.iter()
                    .filter_map(|p| p.type_name.clone())
                    .collect();
                let field_ctor_mangled = mangle_function_name("constructor", &field_param_types);
                overloads.push(field_ctor_mangled.clone());

                methods.insert(field_ctor_mangled.clone(), MethodSignature {
                    name: "constructor".to_string(),
                    params: field_params,
                    return_type: None,
                    return_optional: false,
                    private: false,
                    is_native: false,
                    is_static: false,
                    type_params: Vec::new(),
                    mangled_name: Some(field_ctor_mangled),
                });
            }
        }

        // Build vtable: collect all virtual methods (methods that override interface methods)
        // Use base method names (not mangled) for vtable
        let mut vtable = Vec::new();
        for method_name in method_overloads.keys() {
            vtable.push(method_name.clone());
        }

        self.classes.insert(class.name.clone(), ClassInfo {
            name: class.name.clone(),
            type_params: class.type_params.clone(),
            fields,
            methods,
            method_overloads,
            parent_interfaces: class.parent_interfaces.clone(),
            vtable,
            is_interface: false,
            is_native: class.is_native,
            private: class.private,
        });
    }

    pub fn add_interface(&mut self, interface: &crate::parser::InterfaceDef) {
        let mut methods = HashMap::new();
        let mut method_overloads: HashMap<String, Vec<String>> = HashMap::new();
        for method in &interface.methods {
            let params: Vec<ParamSignature> = method.params.iter().map(|p| ParamSignature {
                name: p.name.clone(),
                type_name: p.type_name.as_ref().map(|t| Type::from_str(t)),
                default: false,
            }).collect();

            // Generate mangled name for the method
            let param_types: Vec<Type> = params.iter()
                .filter_map(|p| p.type_name.clone())
                .collect();
            let mangled = mangle_function_name(&method.name, &param_types);

            let overloads = method_overloads.entry(method.name.clone()).or_insert_with(Vec::new);
            overloads.push(mangled.clone());

            methods.insert(mangled.clone(), MethodSignature {
                name: method.name.clone(),
                params,
                return_type: method.return_type.as_ref().map(|t| Type::from_str(t)),
                return_optional: method.return_optional,
                private: method.private,
                is_native: method.is_native,
                is_static: method.is_static,
                type_params: method.type_params.clone(),
                mangled_name: Some(mangled.clone()),
            });
        }

        // Build vtable: ordered list of method names
        let mut vtable = Vec::new();
        for method_name in method_overloads.keys() {
            vtable.push(method_name.clone());
        }

        self.interfaces.insert(interface.name.clone(), InterfaceInfo {
            name: interface.name.clone(),
            type_params: interface.type_params.clone(),
            parent_interfaces: interface.parent_interfaces.clone(),
            methods,
            method_overloads,
            vtable,
            private: interface.private,
        });
    }

    pub fn add_function(&mut self, name: &str, mut signature: FunctionSignature) {
        // Generate mangled name from parameter types
        let param_types: Vec<Type> = signature.params.iter()
            .filter_map(|p| p.type_name.clone())
            .collect();
        let mangled = mangle_function_name(name, &param_types);
        signature.mangled_name = Some(mangled.clone());

        // Track overload by base name
        self.function_overloads
            .entry(name.to_string())
            .or_insert_with(Vec::new)
            .push(mangled.clone());

        // Store by mangled name
        self.functions.insert(mangled, signature);
    }

    pub fn add_type_alias(&mut self, alias: &crate::parser::TypeAliasDef) {
        let aliased_type = Type::from_str(&alias.aliased_type);
        self.type_aliases.insert(alias.name.clone(), TypeAliasInfo {
            name: alias.name.clone(),
            type_params: alias.type_params.clone(),
            aliased_type,
            private: alias.private,
        });
    }

    pub fn get_type_alias(&self, name: &str) -> Option<&TypeAliasInfo> {
        self.type_aliases.get(name)
    }

    pub fn add_enum(&mut self, enum_def: &crate::parser::EnumDef) {
        let mut variants = HashMap::new();
        let mut next_value: i64 = 0;

        for variant in &enum_def.variants {
            let value = if let Some(expr) = &variant.value {
                if let Expr::Literal(Literal::Int(n, _)) = expr {
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
            private: enum_def.private,
        });
    }

    pub fn get_enum(&self, name: &str) -> Option<&EnumInfo> {
        self.enums.get(name)
    }

    pub fn get_enum_variant(&self, enum_name: &str, variant_name: &str) -> Option<&EnumVariantInfo> {
        self.enums.get(enum_name).and_then(|e| e.variants.get(variant_name))
    }

    pub fn add_variable(&mut self, name: &str, type_name: Type, private: bool) {
        self.variables.insert(name.to_string(), VariableInfo {
            name: name.to_string(),
            type_name,
            private,
        });
    }

    pub fn get_variable(&self, name: &str) -> Option<&VariableInfo> {
        self.variables.get(name)
    }

    pub fn get_class(&self, name: &str) -> Option<&ClassInfo> {
        self.classes.get(name)
    }

    pub fn get_interface(&self, name: &str) -> Option<&InterfaceInfo> {
        self.interfaces.get(name)
    }

    pub fn get_function(&self, name: &str) -> Option<&FunctionSignature> {
        // First try exact match (for mangled names or non-overloaded functions)
        if let Some(sig) = self.functions.get(name) {
            return Some(sig);
        }
        None
    }

    /// Substitute type parameters in a type with actual type arguments
    /// E.g., TypeParameter("T") with mapping {"T": Str} -> Str
    pub fn substitute_type_params(&self, ty: &Type, type_args: &[Type], type_params: &[String]) -> Type {
        match ty {
            Type::TypeParameter(param_name) => {
                // Find the corresponding type argument
                if let Some(idx) = type_params.iter().position(|p| p == param_name) {
                    if let Some(arg) = type_args.get(idx) {
                        return arg.clone();
                    }
                }
                ty.clone()
            }
            Type::Array(inner) => {
                Type::Array(Box::new(self.substitute_type_params(inner, type_args, type_params)))
            }
            Type::Optional(inner) => {
                Type::Optional(Box::new(self.substitute_type_params(inner, type_args, type_params)))
            }
            Type::GenericInstance(name, args) => {
                let substituted_args: Vec<Type> = args
                    .iter()
                    .map(|arg| self.substitute_type_params(arg, type_args, type_params))
                    .collect();
                Type::GenericInstance(name.clone(), substituted_args)
            }
            Type::Function(params, return_type) => {
                let substituted_params: Vec<Type> = params
                    .iter()
                    .map(|p| self.substitute_type_params(p, type_args, type_params))
                    .collect();
                let substituted_return = Box::new(self.substitute_type_params(return_type, type_args, type_params));
                Type::Function(substituted_params, substituted_return)
            }
            _ => ty.clone(),
        }
    }

    /// Get all overloaded versions of a function by base name
    pub fn get_function_overloads(&self, name: &str) -> Vec<&FunctionSignature> {
        let mut result = Vec::new();
        if let Some(mangled_names) = self.function_overloads.get(name) {
            for mangled in mangled_names {
                if let Some(sig) = self.functions.get(mangled) {
                    result.push(sig);
                }
            }
        }
        result
    }

    /// Try to resolve a function name, including searching for unqualified names
    /// in qualified functions (e.g., "foo" matches "std::sys::foo")
    /// For overloaded functions, returns the first match (full resolution requires type checking)
    pub fn resolve_function(&self, name: &str) -> Option<&FunctionSignature> {
        // First try exact match (mangled name)
        if let Some(sig) = self.functions.get(name) {
            return Some(sig);
        }

        // Try to find overloads by base name
        if let Some(mangled_names) = self.function_overloads.get(name) {
            if let Some(first_mangled) = mangled_names.first() {
                if let Some(sig) = self.functions.get(first_mangled) {
                    // Check visibility for private functions
                    if sig.private {
                        // Private functions are only visible within the same module
                        if let Some(ref current_module) = self.current_module {
                            let func_module = sig.name.rsplit('.').nth(1).unwrap_or("");
                            if func_module != *current_module {
                                return None; // Private function not visible from this module
                            }
                        } else {
                            return None; // Private function not visible from global scope
                        }
                    }
                    return Some(sig);
                }
            }
        }

        // Try to find a function that ends with ::<name> or .<name>
        // Prefer exact module match if we're in a module context
        // First, collect all matching functions
        let mut matching_sigs: Vec<&FunctionSignature> = Vec::new();
        for (func_name, sig) in &self.functions {
            if func_name.ends_with(&format!(".{}", name)) {
                matching_sigs.push(sig);
            }
        }

        // If we have matches, find the best one based on imports and visibility
        if !matching_sigs.is_empty() {
            // First, try to find a match from an imported module that is not private
            for import_entry in &self.imports {
                for sig in &matching_sigs {
                    // Check if function is from the imported module
                    let from_imported_module = match import_entry.kind {
                        crate::parser::ImportKind::Module => {
                            // Module import: function should be under import_entry.module_path.*
                            sig.name.starts_with(&format!("{}.", import_entry.module_path))
                        }
                        crate::parser::ImportKind::Simple | crate::parser::ImportKind::Aliased(_) => {
                            // Simple/aliased import: function should be under the module path
                            // Also check if the function name is in the members list (for wildcard-like behavior)
                            let from_module = sig.name.starts_with(&format!("{}.", import_entry.module_path));
                            let from_members = import_entry.members.iter().any(|m| {
                                sig.name.ends_with(&format!(".{}", m)) || sig.name == *m
                            });
                            from_module || from_members
                        }
                        crate::parser::ImportKind::Member => {
                            // Member import: function name should match the alias
                            if let Some(ref alias) = import_entry.alias {
                                sig.name.ends_with(&format!(".{}", alias))
                            } else {
                                false
                            }
                        }
                        crate::parser::ImportKind::Wildcard => {
                            // Wildcard: function should be directly under the module path
                            // or in the members list
                            let from_module = sig.name.starts_with(&format!("{}.", import_entry.module_path));
                            let from_members = import_entry.members.iter().any(|m| {
                                sig.name.ends_with(&format!(".{}", m)) || sig.name == *m
                            });
                            from_module || from_members
                        }
                    };

                    if from_imported_module {
                        // Check visibility
                        if sig.private {
                            // Private functions are only visible within the same module
                            if let Some(ref current_module) = self.current_module {
                                if import_entry.module_path == *current_module {
                                    return Some(sig); // Visible - same module
                                }
                            }
                            // Not visible from this module
                            continue;
                        }
                        return Some(sig); // Public function from imported module
                    }
                }
            }

            // If no imported match found, return the first non-private match
            for sig in &matching_sigs {
                if !sig.private {
                    return Some(sig);
                }
            }
        }

        None
    }

    /// Try to resolve a module-qualified function name (e.g., "math.sin" with import "std.math")
    pub fn resolve_qualified_function(&self, module_alias: &str, member_name: &str) -> Option<&FunctionSignature> {
        // First try direct lookup (e.g., "std.io.println" when user wrote "io.println")
        let direct_name = format!("{}.{}", module_alias, member_name);
        if let Some(mangled_names) = self.function_overloads.get(&direct_name) {
            if let Some(first_mangled) = mangled_names.first() {
                if let Some(sig) = self.functions.get(first_mangled) {
                    // Check visibility: private functions are only visible within the same module
                    if sig.private {
                        if let Some(ref current_module) = self.current_module {
                            let func_module = sig.name.rsplit('.').nth(1).unwrap_or("");
                            if func_module != *current_module {
                                return None;
                            }
                        } else {
                            return None;
                        }
                    }
                    return Some(sig);
                }
            }
        }

        // Try to find an import that matches the module alias
        for import_entry in &self.imports {
            let module_matches = match &import_entry.alias {
                Some(alias) => alias == module_alias,
                None => {
                    // No alias - check if the last component of the module path matches
                    import_entry.module_path.ends_with(&format!(".{}", module_alias))
                        || import_entry.module_path == module_alias
                }
            };

            if module_matches {
                // Build the qualified name based on import kind
                let qualified_name = match import_entry.kind {
                    crate::parser::ImportKind::Module => {
                        // import std -> std.io.println
                        format!("{}.{}.{}", import_entry.module_path, module_alias, member_name)
                    }
                    crate::parser::ImportKind::Simple | crate::parser::ImportKind::Aliased(_) => {
                        // import std.io -> std.io.println
                        format!("{}.{}", import_entry.module_path, member_name)
                    }
                    crate::parser::ImportKind::Member => {
                        // import std.io.println - shouldn't reach here for qualified access
                        continue;
                    }
                    crate::parser::ImportKind::Wildcard => {
                        // import std.io.* -> std.io.println
                        format!("{}.{}", import_entry.module_path, member_name)
                    }
                };

                if let Some(mangled_names) = self.function_overloads.get(&qualified_name) {
                    if let Some(first_mangled) = mangled_names.first() {
                        if let Some(sig) = self.functions.get(first_mangled) {
                            // Check visibility
                            if sig.private {
                                if let Some(ref current_module) = self.current_module {
                                    if !import_entry.module_path.ends_with(&format!(".{}", current_module)) 
                                        && import_entry.module_path != *current_module {
                                        return None;
                                    }
                                } else {
                                    return None;
                                }
                            }
                            return Some(sig);
                        }
                    }
                }
            }
        }

        // Fallback: check import_paths for backward compatibility
        for import_path in &self.import_paths {
            if let Some(last_dot) = import_path.rfind('.') {
                let import_alias = &import_path[last_dot + 1..];
                if import_alias == module_alias {
                    let qualified_name = format!("{}.{}", import_path, member_name);
                    if let Some(mangled_names) = self.function_overloads.get(&qualified_name) {
                        if let Some(first_mangled) = mangled_names.first() {
                            if let Some(sig) = self.functions.get(first_mangled) {
                                if sig.private {
                                    if let Some(ref current_module) = self.current_module {
                                        if !import_path.ends_with(&format!(".{}", current_module)) && *import_path != *current_module {
                                            return None;
                                        }
                                    } else {
                                        return None;
                                    }
                                }
                                return Some(sig);
                            }
                        }
                    }
                }
            }

            let qualified_name = format!("{}.{}.{}", import_path, module_alias, member_name);
            if let Some(mangled_names) = self.function_overloads.get(&qualified_name) {
                if let Some(first_mangled) = mangled_names.first() {
                    if let Some(sig) = self.functions.get(first_mangled) {
                        if sig.private {
                            if let Some(ref current_module) = self.current_module {
                                if !qualified_name.ends_with(&format!(".{}", current_module)) {
                                    return None;
                                }
                            } else {
                                return None;
                            }
                        }
                        return Some(sig);
                    }
                }
            }
        }

        None
    }

    /// Try to resolve a module-qualified class name (e.g., "http.HttpClient" with import "std.http")
    pub fn resolve_qualified_class(&self, module_alias: &str, class_name: &str) -> Option<String> {
        // First, check if this is a nested class (e.g., Outer.Inner)
        // module_alias would be "Outer" and class_name would be "Inner"
        let nested_class_name = format!("{}.{}", module_alias, class_name);
        if let Some(class_info) = self.classes.get(&nested_class_name) {
            if class_info.private {
                // Check if we're inside the outer class
                if let Some(ref current_class) = self.current_class {
                    if current_class != module_alias {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            return Some(nested_class_name);
        }

        // First try direct lookup for module-qualified classes
        let direct_name = format!("{}.{}", module_alias, class_name);
        if let Some(class_info) = self.classes.get(&direct_name) {
            if class_info.private {
                if let Some(ref current_module) = self.current_module {
                    let class_module = direct_name.rsplit('.').nth(1).unwrap_or("");
                    if class_module != *current_module {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            return Some(direct_name);
        }

        // Try to find an import that matches the module alias
        for import_entry in &self.imports {
            let module_matches = match &import_entry.alias {
                Some(alias) => alias == module_alias,
                None => {
                    import_entry.module_path.ends_with(&format!(".{}", module_alias))
                        || import_entry.module_path == module_alias
                }
            };

            if module_matches {
                let qualified_name = match import_entry.kind {
                    crate::parser::ImportKind::Module => {
                        format!("{}.{}.{}", import_entry.module_path, module_alias, class_name)
                    }
                    crate::parser::ImportKind::Simple | crate::parser::ImportKind::Aliased(_) => {
                        format!("{}.{}", import_entry.module_path, class_name)
                    }
                    crate::parser::ImportKind::Member => {
                        continue;
                    }
                    crate::parser::ImportKind::Wildcard => {
                        format!("{}.{}", import_entry.module_path, class_name)
                    }
                };

                if let Some(class_info) = self.classes.get(&qualified_name) {
                    if class_info.private {
                        if let Some(ref current_module) = self.current_module {
                            if !import_entry.module_path.ends_with(&format!(".{}", current_module)) 
                                && import_entry.module_path != *current_module {
                                return None;
                            }
                        } else {
                            return None;
                        }
                    }
                    return Some(qualified_name);
                }
            }
        }

        // Fallback to import_paths for backward compatibility
        for import_path in &self.import_paths {
            if let Some(last_dot) = import_path.rfind('.') {
                let import_alias = &import_path[last_dot + 1..];
                if import_alias == module_alias {
                    let qualified_name = format!("{}.{}", import_path, class_name);
                    if let Some(class_info) = self.classes.get(&qualified_name) {
                        if class_info.private {
                            if let Some(ref current_module) = self.current_module {
                                if !import_path.ends_with(&format!(".{}", current_module)) && *import_path != *current_module {
                                    return None;
                                }
                            } else {
                                return None;
                            }
                        }
                        return Some(qualified_name);
                    }
                }
            } else if import_path == module_alias {
                let qualified_name = format!("{}.{}", import_path, class_name);
                if let Some(class_info) = self.classes.get(&qualified_name) {
                    if class_info.private {
                        if let Some(ref current_module) = self.current_module {
                            if *import_path != *current_module {
                                return None;
                            }
                        } else {
                            return None;
                        }
                    }
                    return Some(qualified_name);
                }
            }

            let qualified_name = format!("{}.{}.{}", import_path, module_alias, class_name);
            if let Some(class_info) = self.classes.get(&qualified_name) {
                if class_info.private {
                    if let Some(ref current_module) = self.current_module {
                        if !qualified_name.ends_with(&format!(".{}", current_module)) {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }
                return Some(qualified_name);
            }
        }

        None
    }

    /// Try to resolve a module-qualified variable name (e.g., "math.PI" with import "std.math")
    pub fn resolve_qualified_variable(&self, module_alias: &str, var_name: &str) -> Option<&VariableInfo> {
        // First try direct lookup
        let direct_name = format!("{}.{}", module_alias, var_name);
        if let Some(var) = self.variables.get(&direct_name) {
            if var.private {
                if let Some(ref current_module) = self.current_module {
                    let var_module = var.name.rsplit('.').nth(1).unwrap_or("");
                    if var_module != *current_module {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            return Some(var);
        }

        // Try to find an import that matches the module alias
        for import_entry in &self.imports {
            let module_matches = match &import_entry.alias {
                Some(alias) => alias == module_alias,
                None => {
                    import_entry.module_path.ends_with(&format!(".{}", module_alias))
                        || import_entry.module_path == module_alias
                }
            };

            if module_matches {
                let qualified_name = match import_entry.kind {
                    crate::parser::ImportKind::Module => {
                        format!("{}.{}.{}", import_entry.module_path, module_alias, var_name)
                    }
                    crate::parser::ImportKind::Simple | crate::parser::ImportKind::Aliased(_) => {
                        format!("{}.{}", import_entry.module_path, var_name)
                    }
                    crate::parser::ImportKind::Member => {
                        continue;
                    }
                    crate::parser::ImportKind::Wildcard => {
                        format!("{}.{}", import_entry.module_path, var_name)
                    }
                };

                if let Some(var) = self.variables.get(&qualified_name) {
                    if var.private {
                        if let Some(ref current_module) = self.current_module {
                            if !import_entry.module_path.ends_with(&format!(".{}", current_module))
                                && import_entry.module_path != *current_module {
                                return None;
                            }
                        } else {
                            return None;
                        }
                    }
                    return Some(var);
                }
            }
        }

        // Fallback to import_paths for backward compatibility
        for import_path in &self.import_paths {
            if let Some(last_dot) = import_path.rfind('.') {
                let import_alias = &import_path[last_dot + 1..];
                if import_alias == module_alias {
                    let qualified_name = format!("{}.{}", import_path, var_name);
                    if let Some(var) = self.variables.get(&qualified_name) {
                        if var.private {
                            if let Some(ref current_module) = self.current_module {
                                if !import_path.ends_with(&format!(".{}", current_module)) && *import_path != *current_module {
                                    return None;
                                }
                            } else {
                                return None;
                            }
                        }
                        return Some(var);
                    }
                }
            } else if import_path == module_alias {
                let qualified_name = format!("{}.{}", import_path, var_name);
                if let Some(var) = self.variables.get(&qualified_name) {
                    if var.private {
                        if let Some(ref current_module) = self.current_module {
                            if *import_path != *current_module {
                                return None;
                            }
                        } else {
                            return None;
                        }
                    }
                    return Some(var);
                }
            }

            let qualified_name = format!("{}.{}.{}", import_path, module_alias, var_name);
            if let Some(var) = self.variables.get(&qualified_name) {
                if var.private {
                    if let Some(ref current_module) = self.current_module {
                        if !qualified_name.ends_with(&format!(".{}", current_module)) {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }
                return Some(var);
            }
        }

        None
    }

    /// Resolve a function call with argument types for overload resolution
    /// Returns the best matching function signature based on argument types
    pub fn resolve_function_call(&self, name: &str, arg_types: &[Type]) -> Option<&FunctionSignature> {
        // First try exact match with mangled name (if already resolved)
        if let Some(sig) = self.functions.get(name) {
            return Some(sig);
        }

        // Helper to find best match among overloads
        let find_best_match = |overloads: &[String]| -> Option<&FunctionSignature> {
            let mut best_match: Option<&FunctionSignature> = None;
            let mut best_score = usize::MAX;

            for mangled in overloads {
                if let Some(sig) = self.functions.get(mangled) {
                    if self.signature_matches(sig, arg_types) {
                        let score = self.calculate_match_score(sig, arg_types);
                        if score < best_score {
                            best_score = score;
                            best_match = Some(sig);
                        }
                    }
                }
            }
            best_match
        };

        // 1. Try as a full name (with overloads)
        if let Some(overloads) = self.function_overloads.get(name) {
            if let Some(res) = find_best_match(overloads) {
                // Check visibility for private functions
                if res.private {
                    if let Some(ref current_module) = self.current_module {
                        let func_module = res.name.rsplit('.').nth(1).unwrap_or("");
                        if func_module != *current_module {
                            return None; // Private function not visible from this module
                        }
                    } else {
                        return None; // Private function not visible from global scope
                    }
                }
                return Some(res);
            }
        }

        // 2. Try with imports
        for import_entry in &self.imports {
            let qualified = format!("{}.{}", import_entry.module_path, name);
            if let Some(overloads) = self.function_overloads.get(&qualified) {
                if let Some(res) = find_best_match(overloads) {
                    // Check visibility for private functions
                    if res.private {
                        if let Some(ref current_module) = self.current_module {
                            if import_entry.module_path != *current_module {
                                continue; // Private function not visible from this module
                            }
                        } else {
                            continue; // Private function not visible from global scope
                        }
                    }
                    return Some(res);
                }
            }

            // Also try parent module sub-access (e.g., import std and access io.println -> std.io.println)
            if let Some(dot_pos) = name.find('.') {
                let (prefix, rest) = name.split_at(dot_pos);
                let qualified2 = format!("{}.{}.{}", import_entry.module_path, prefix, &rest[1..]);
                if let Some(overloads) = self.function_overloads.get(&qualified2) {
                    if let Some(res) = find_best_match(overloads) {
                        // Check visibility for private functions
                        if res.private {
                            if let Some(ref current_module) = self.current_module {
                                if import_entry.module_path != *current_module {
                                    continue; // Private function not visible from this module
                                }
                            } else {
                                continue; // Private function not visible from global scope
                            }
                        }
                        return Some(res);
                    }
                }
            }
        }

        // 3. Try qualified lookup (fallback for older code)
        // Only consider functions from imported modules or the current module
        for (func_mangled, sig) in &self.functions {
            // Extract base name from mangled name (part before '(')
            let base_name = match func_mangled.find('(') {
                Some(pos) => &func_mangled[..pos],
                None => func_mangled,
            };

            if base_name.ends_with(&format!(".{}", name)) || base_name.ends_with(&format!("::{}", name)) {
                if self.signature_matches(sig, arg_types) {
                    // Check if this function is from an imported module or the current module
                    let from_imported_module = self.imports.iter().any(|import_entry| {
                        base_name.starts_with(&format!("{}.", import_entry.module_path)) || base_name.starts_with(&format!("{}::", import_entry.module_path))
                    });

                    let from_current_module = if let Some(ref current_module) = self.current_module {
                        base_name.starts_with(&format!("{}.", current_module)) || base_name.starts_with(&format!("{}::", current_module))
                    } else {
                        false
                    };
                    
                    if !from_imported_module && !from_current_module {
                        continue; // Skip functions from non-imported modules
                    }
                    
                    // Check visibility for private functions
                    if sig.private {
                        if let Some(ref current_module) = self.current_module {
                            let func_module = sig.name.rsplit('.').nth(1).unwrap_or("");
                            if func_module != *current_module {
                                continue; // Private function not visible from this module
                            }
                        } else {
                            continue; // Private function not visible from global scope
                        }
                    }
                    return Some(sig);
                }
            }
        }

        None
    }

    /// Check if a function signature matches the given argument types
    fn signature_matches(&self, sig: &FunctionSignature, arg_types: &[Type]) -> bool {
        // Allow calling with fewer arguments if remaining params have defaults
        if arg_types.len() > sig.params.len() {
            return false; // Too many arguments
        }
        
        // Check if we have enough params (including defaults) for the provided args
        let min_args = sig.params.iter().position(|p| p.default).unwrap_or(sig.params.len());
        if arg_types.len() < min_args {
            return false; // Not enough arguments (before first default)
        }

        for (param, arg_type) in sig.params.iter().zip(arg_types.iter()) {
            if let Some(param_type) = &param.type_name {
                // Allow Unknown types to match anything
                if param_type != &Type::Unknown && arg_type != &Type::Unknown {
                    if !arg_type.is_assignable_to(param_type) {
                        return false;
                    }
                }
            }
            // If param has no type, it matches anything
        }

        true
    }

    /// Calculate a match score (lower is better) for overload resolution
    fn calculate_match_score(&self, sig: &FunctionSignature, arg_types: &[Type]) -> usize {
        let mut score = 0;
        for (param, arg_type) in sig.params.iter().zip(arg_types.iter()) {
            if let Some(param_type) = &param.type_name {
                if param_type == arg_type {
                    score += 0; // Exact match
                } else if arg_type.is_assignable_to(param_type) {
                    score += 1; // Conversion needed
                } else {
                    score += 100; // Bad match (should have been filtered)
                }
            }
        }
        score
    }

    /// Get all available overloads for a function name (including from imports)
    pub fn get_available_overloads(&self, name: &str, _arg_types: &[Type]) -> Vec<&FunctionSignature> {
        let mut overloads = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Helper to add overloads from a list of mangled names
        let mut add_overloads = |overload_list: &[String]| {
            for mangled in overload_list {
                if let Some(sig) = self.functions.get(mangled) {
                    if !seen.contains(&sig.name) {
                        seen.insert(sig.name.clone());
                        overloads.push(sig);
                    }
                }
            }
        };

        // 1. Try direct overloads
        if let Some(overload_list) = self.function_overloads.get(name) {
            add_overloads(overload_list);
        }

        // 2. Try from imports
        for import_entry in &self.imports {
            let qualified = format!("{}.{}", import_entry.module_path, name);
            if let Some(overload_list) = self.function_overloads.get(&qualified) {
                add_overloads(overload_list);
            }

            // Also try parent module sub-access (e.g., import std and access io.println)
            if let Some(dot_pos) = name.find('.') {
                let (prefix, rest) = name.split_at(dot_pos);
                let qualified2 = format!("{}.{}.{}", import_entry.module_path, prefix, &rest[1..]);
                if let Some(overload_list) = self.function_overloads.get(&qualified2) {
                    add_overloads(overload_list);
                }
            }
        }

        // 3. Try qualified lookup (functions ending with .name)
        for (func_mangled, sig) in &self.functions {
            let base_name = match func_mangled.find('(') {
                Some(pos) => &func_mangled[..pos],
                None => func_mangled,
            };

            if base_name.ends_with(&format!(".{}", name)) || base_name.ends_with(&format!("::{}", name)) {
                if !seen.contains(&sig.name) {
                    seen.insert(sig.name.clone());
                    overloads.push(sig);
                }
            }
        }

        overloads
    }

    /// Generate a detailed error message for no matching overload (Clang-style)
    pub fn format_no_matching_overload_error(&self, name: &str, arg_types: &[Type], _span_line: usize, _span_column: usize) -> String {
        let overloads = self.get_available_overloads(name, arg_types);
        
        if overloads.is_empty() {
            // No overloads found at all - function doesn't exist
            return format!("Undefined function: '{}'", name);
        }

        // We found overloads, but none match - report them like Clang does
        let mut msg = String::new();
        msg.push_str(&format!("No matching function '{}' for arguments: (", name));
        
        // Show the argument types that were provided
        let arg_types_str: Vec<String> = arg_types.iter().map(|t| t.to_str()).collect();
        msg.push_str(&arg_types_str.join(", "));
        msg.push_str(")\n");
        
        // List candidate overloads
        msg.push_str("candidate function");
        if overloads.len() != 1 {
            msg.push_str("s");
        }
        msg.push_str(" not viable:\n");
        for sig in &overloads {
            let param_types: Vec<String> = sig.params.iter()
                .map(|p| {
                    if let Some(ref t) = p.type_name {
                        t.to_str().to_string()
                    } else {
                        "<no type>".to_string()
                    }
                })
                .collect();
            msg.push_str(&format!("{}({}) [unconvertible]\n", name, param_types.join(", ")));
        }
        
        msg
    }

    /// Try to resolve a class name, including searching for unqualified names
    /// in qualified classes (e.g., "HttpClient" matches "std::http::HttpClient")
    pub fn resolve_class(&self, name: &str) -> Option<String> {
        // First try exact match
        if let Some(class_info) = self.classes.get(name) {
            // Check visibility for private classes
            if class_info.private {
                if let Some(ref current_module) = self.current_module {
                    let class_module = name.rsplit('.').nth(1).unwrap_or("");
                    if class_module != *current_module {
                        return None; // Private class not visible from this module
                    }
                } else {
                    return None; // Private class not visible from global scope
                }
            }
            // If the exact match contains :: or ., use it; otherwise check if there's also a qualified version
            let exact = name.to_string();
            if exact.contains("::") || exact.contains('.') {
                return Some(exact);
            }
            // Prefer qualified version if available (try :: first, then .)
            for class_name in self.classes.keys() {
                if class_name.ends_with(&format!("::{}", name)) {
                    if let Some(ci) = self.classes.get(class_name) {
                        if ci.private {
                            if let Some(ref current_module) = self.current_module {
                                let class_module = class_name.rsplit('.').nth(1).unwrap_or("");
                                if class_module != *current_module {
                                    continue; // Private class not visible
                                }
                            } else {
                                continue; // Private class not visible from global scope
                            }
                        }
                    }
                    return Some(class_name.clone());
                }
            }
            for class_name in self.classes.keys() {
                if class_name.ends_with(&format!(".{}", name)) {
                    if let Some(ci) = self.classes.get(class_name) {
                        if ci.private {
                            if let Some(ref current_module) = self.current_module {
                                let class_module = class_name.rsplit('.').nth(1).unwrap_or("");
                                if class_module != *current_module {
                                    continue; // Private class not visible
                                }
                            } else {
                                continue; // Private class not visible from global scope
                            }
                        }
                    }
                    return Some(class_name.clone());
                }
            }
            return Some(exact);
        }

        // Try to find a class that ends with ::<name>
        for class_name in self.classes.keys() {
            if class_name.ends_with(&format!("::{}", name)) {
                if let Some(ci) = self.classes.get(class_name) {
                    if ci.private {
                        if let Some(ref current_module) = self.current_module {
                            let class_module = class_name.rsplit('.').nth(1).unwrap_or("");
                            if class_module != *current_module {
                                continue; // Private class not visible
                            }
                        } else {
                            continue; // Private class not visible from global scope
                        }
                    }
                }
                return Some(class_name.clone());
            }
        }

        // Try to find a class that ends with .<name>
        // Only consider classes from imported modules or the current module
        for class_name in self.classes.keys() {
            if class_name.ends_with(&format!(".{}", name)) {
                if let Some(ci) = self.classes.get(class_name) {
                    // Check if this class is from an imported module or the current module
                    let from_imported_module = self.imports.iter().any(|import_entry| {
                        class_name.starts_with(&format!("{}.", import_entry.module_path)) || class_name.starts_with(&format!("{}::", import_entry.module_path))
                    });

                    let from_current_module = if let Some(ref current_module) = self.current_module {
                        class_name.starts_with(&format!("{}.", current_module)) || class_name.starts_with(&format!("{}::", current_module))
                    } else {
                        false
                    };
                    
                    if !from_imported_module && !from_current_module {
                        continue; // Skip classes from non-imported modules
                    }
                    
                    // Check visibility for private classes
                    if ci.private {
                        if let Some(ref current_module) = self.current_module {
                            let class_module = class_name.rsplit('.').nth(1).unwrap_or("");
                            if class_module != *current_module {
                                continue; // Private class not visible from this module
                            }
                        } else {
                            continue; // Private class not visible from global scope
                        }
                    }
                }
                return Some(class_name.clone());
            }
        }

        None
    }

    pub fn get_method(&self, class_name: &str, method_name: &str) -> Option<&MethodSignature> {
        self.classes.get(class_name).and_then(|c| c.methods.get(method_name))
    }

    /// Resolve a method call with argument types for overload resolution
    /// Returns the best matching method signature based on argument types
    pub fn resolve_method_call(&self, class_name: &str, method_name: &str, arg_types: &[Type]) -> Option<&MethodSignature> {
        let class_info = self.classes.get(class_name)?;
        
        // Helper to find best match among overloads
        let find_best_match = |overloads: &[String]| -> Option<&MethodSignature> {
            let mut best_match: Option<&MethodSignature> = None;
            let mut best_score = usize::MAX;

            for mangled in overloads {
                if let Some(sig) = class_info.methods.get(mangled) {
                    if self.method_signature_matches(sig, arg_types) {
                        let score = self.calculate_method_match_score(sig, arg_types);
                        if score < best_score {
                            best_score = score;
                            best_match = Some(sig);
                        }
                    }
                }
            }
            best_match
        };

        // Try to find overloads for this method
        if let Some(overloads) = class_info.method_overloads.get(method_name) {
            return find_best_match(overloads);
        }

        None
    }

    /// Check if two types are compatible (assignable)
    fn types_compatible(&self, from_type: &Type, to_type: &Type) -> bool {
        // Exact match
        if from_type == to_type {
            return true;
        }
        
        // Unknown type is compatible with anything
        if *from_type == Type::Unknown || *to_type == Type::Unknown {
            return true;
        }
        
        // Null is compatible with Optional types
        if *from_type == Type::Null {
            if let Type::Optional(_) = to_type {
                return true;
            }
        }
        
        // Numeric type compatibility
        match (from_type, to_type) {
            (Type::Int, Type::Float) => true,
            (Type::Int8, Type::Int) | (Type::Int8, Type::Int16) | (Type::Int8, Type::Int32) | (Type::Int8, Type::Int64) => true,
            (Type::Int16, Type::Int) | (Type::Int16, Type::Int32) | (Type::Int16, Type::Int64) => true,
            (Type::Int32, Type::Int) | (Type::Int32, Type::Int64) => true,
            (Type::UInt8, Type::UInt16) | (Type::UInt8, Type::UInt32) | (Type::UInt8, Type::UInt64) => true,
            (Type::UInt16, Type::UInt32) | (Type::UInt16, Type::UInt64) => true,
            (Type::UInt32, Type::UInt64) => true,
            (Type::Float32, Type::Float64) | (Type::Float32, Type::Float) => true,
            (Type::Float64, Type::Float) => true,
            _ => false,
        }
    }

    /// Check if a method signature matches the given argument types
    fn method_signature_matches(&self, sig: &MethodSignature, arg_types: &[Type]) -> bool {
        if sig.params.len() != arg_types.len() {
            return false;
        }

        for (param, arg_type) in sig.params.iter().zip(arg_types.iter()) {
            if let Some(ref param_type) = param.type_name {
                if !self.types_compatible(arg_type, param_type) {
                    return false;
                }
            }
        }

        true
    }

    /// Calculate match score for method overload resolution (lower is better)
    fn calculate_method_match_score(&self, sig: &MethodSignature, arg_types: &[Type]) -> usize {
        let mut score = 0;
        for (param, arg_type) in sig.params.iter().zip(arg_types.iter()) {
            if let Some(ref param_type) = param.type_name {
                if arg_type == param_type {
                    score += 1; // Exact match
                } else if self.types_compatible(arg_type, param_type) {
                    score += 2; // Compatible but not exact
                } else {
                    score += 100; // Incompatible (shouldn't happen if signature_matches returned true)
                }
            }
        }
        score
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

    /// Parse a function name that may include generic type arguments
    /// E.g., "identity<T>" -> ("identity", vec!["T"])
    /// E.g., "wrap<T, U>" -> ("wrap", vec!["T", "U"])  
    /// E.g., "foo" -> ("foo", vec![])
    fn parse_generic_function_name(name: &str) -> (&str, Vec<String>) {
        if let Some(angle_start) = name.find('<') {
            if name.ends_with('>') {
                let base_name = &name[..angle_start];
                let args_str = &name[angle_start + 1..name.len() - 1];
                let args: Vec<String> = args_str.split(',').map(|s| s.trim().to_string()).collect();
                return (base_name, args);
            }
        }
        (name, Vec::new())
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

    /// Get span information from an expression for error reporting
    fn get_expr_span(expr: &Expr) -> (usize, usize) {
        match expr {
            Expr::Literal(lit) => match lit {
                Literal::String(_, span) => (span.line, span.column),
                Literal::Int(_, span) => (span.line, span.column),
                Literal::Float(_, span) => (span.line, span.column),
                Literal::Bool(_, span) => (span.line, span.column),
                Literal::Null(span) => (span.line, span.column),
            },
            Expr::Variable { span, .. } => (span.line, span.column),
            Expr::Binary { span, .. } => (span.line, span.column),
            Expr::Unary { span, .. } => (span.line, span.column),
            Expr::Call { span, .. } => (span.line, span.column),
            Expr::Get { span, .. } => (span.line, span.column),
            Expr::Set { span, .. } => (span.line, span.column),
            Expr::Interpolated { span, .. } => (span.line, span.column),
            Expr::Range { span, .. } => (span.line, span.column),
            Expr::Cast { span, .. } => (span.line, span.column),
            Expr::Array { span, .. } => (span.line, span.column),
            Expr::Index { span, .. } => (span.line, span.column),
            Expr::ObjectLiteral { span, .. } => (span.line, span.column),
            Expr::Lambda { span, .. } => (span.line, span.column),
        }
    }

    fn collect_definitions(&mut self, statements: &[Stmt], skip_functions: bool) {
        for stmt in statements {
            match stmt {
                Stmt::Class(class) => {
                    self.add_class_with_nesting(class, None);
                }
                Stmt::Interface(interface) => {
                    self.add_interface_with_nesting(interface, None);
                }
                Stmt::Enum(enum_def) => {
                    self.context.add_enum(enum_def);
                }
                Stmt::TypeAlias(alias) => {
                    self.context.add_type_alias(alias);
                }
                Stmt::Function(func) => {
                    if !skip_functions {
                        let params: Vec<ParamSignature> = func.params.iter().map(|p| ParamSignature {
                            name: p.name.clone(),
                            type_name: p.type_name.as_ref().map(|t| Type::from_str(t)),
                            default: p.default.is_some(),
                        }).collect();

                        self.context.add_function(&func.name, FunctionSignature {
                            name: func.name.clone(),
                            params,
                            return_type: func.return_type.as_ref().map(|t| Type::from_str(t)),
                            return_optional: func.return_optional,
                            is_method: false,
                            is_native: func.is_native,
                            private: func.private,
                            type_params: func.type_params.clone(),
                            mangled_name: None,
                        });
                    }
                }
                Stmt::Import { .. } => {
                    // Import handled during module resolution
                }
                _ => {}
            }
        }
    }

    fn add_class_with_nesting(&mut self, class: &crate::parser::ClassDef, parent_name: Option<&str>) {
        // Set the qualified name for the class
        let mut qualified_class = class.clone();
        if let Some(parent) = parent_name {
            qualified_class.name = format!("{}.{}", parent, class.name);
        }
        
        self.context.add_class(&qualified_class);
        
        // Process nested classes with the qualified parent name
        let current_parent = qualified_class.name.clone();
        for nested_class in &class.nested_classes {
            self.add_class_with_nesting(nested_class, Some(&current_parent));
        }
        // Process nested interfaces
        for nested_iface in &class.nested_interfaces {
            self.add_interface_with_nesting(nested_iface, Some(&current_parent));
        }
    }

    fn add_interface_with_nesting(&mut self, interface: &crate::parser::InterfaceDef, parent_name: Option<&str>) {
        // Set the qualified name for the interface
        let mut qualified_interface = interface.clone();
        if let Some(parent) = parent_name {
            qualified_interface.name = format!("{}.{}", parent, interface.name);
        }
        
        self.context.add_interface(&qualified_interface);
        
        // Process nested classes with the qualified parent name
        let current_parent = qualified_interface.name.clone();
        for nested_class in &interface.nested_classes {
            self.add_class_with_nesting(nested_class, Some(&current_parent));
        }
        // Process nested interfaces
        for nested_iface in &interface.nested_interfaces {
            self.add_interface_with_nesting(nested_iface, Some(&current_parent));
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Module { path: _, .. } => {
                // Module declaration - just for namespacing
            }
            Stmt::Import { .. } => {
                // Import handled in collect_definitions
            }
            Stmt::Class(class) => {
                self.check_class(class);
            }
            Stmt::Interface(_) => {
                // Interface definitions are already processed in collect_definitions
            }
            Stmt::Enum(_) => {
                // Enum definitions are already processed in collect_definitions
            }
            Stmt::TypeAlias(_) => {
                // Type aliases are already processed in collect_definitions
            }
            Stmt::Function(func) => {
                self.check_function(func);
            }
            Stmt::Let { name, type_annotation, expr, private, .. } => {
                // If there's a type annotation, use it for type deduction
                let expr_type = if let Some(ref type_name) = type_annotation {
                    let expected_type = Type::from_str(type_name);
                    self.infer_expr_with_expected_type(expr, &Some(expected_type))
                } else {
                    self.infer_expr(expr)
                };
                self.context.add_variable(name, expr_type, *private);
            }
            Stmt::Assign { name, expr, span } => {
                let expr_type = self.infer_expr(expr);

                if let Some(var_info) = self.context.get_variable(name) {
                    // Use expected type for type deduction
                    let expected_type = var_info.type_name.clone();
                    let deduced_type = self.infer_expr_with_expected_type(expr, &Some(expected_type.clone()));

                    if !deduced_type.is_assignable_to(&expected_type) {
                        self.context.add_error_with_location(
                            format!(
                                "Type mismatch: cannot assign {} to variable '{}' of type {}",
                                deduced_type.to_str(),
                                name,
                                expected_type.to_str()
                            ),
                            span.line,
                            span.column,
                            None,
                            None,
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
                    self.context.add_variable(name, expr_type, false);
                }
            }
            Stmt::AugAssign { target, op, expr, span } => {
                let var_type = match target {
                    crate::parser::AugAssignTarget::Variable(name) => {
                        if let Some(var_info) = self.context.get_variable(name) {
                            var_info.type_name.clone()
                        } else {
                            self.context.add_error_with_location(
                                format!("Undeclared variable '{}'", name),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                            return;
                        }
                    }
                    crate::parser::AugAssignTarget::Field { object, name } => {
                        let obj_type = self.infer_expr(object);
                        // Get the field type from the object's class
                        if let Type::Class(class_name) = obj_type {
                            if let Some(class_info) = self.context.get_class(&class_name) {
                                if let Some(field_info) = class_info.fields.get(name.as_str()) {
                                    field_info.type_name.clone()
                                } else {
                                    self.context.add_error_with_location(
                                        format!("Field '{}' not found in class '{}'", name, class_name),
                                        span.line,
                                        span.column,
                                        None,
                                        None,
                                    );
                                    return;
                                }
                            } else {
                                self.context.add_error_with_location(
                                    format!("Unknown class '{}'", class_name),
                                    span.line,
                                    span.column,
                                    None,
                                    None,
                                );
                                return;
                            }
                        } else {
                            self.context.add_error_with_location(
                                format!("Cannot access field '{}' on non-class type '{}'", name, obj_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                            return;
                        }
                    }
                };

                let expr_type = self.infer_expr(expr);

                // For +=, -=, *=, /=, %=, &=, |=, ^=, <<=, >>=, both operands must be numeric types
                let op_name = match op {
                    crate::parser::AugOp::Add => "+=",
                    crate::parser::AugOp::Subtract => "-=",
                    crate::parser::AugOp::Multiply => "*=",
                    crate::parser::AugOp::Divide => "/=",
                    crate::parser::AugOp::Modulo => "%=",
                    crate::parser::AugOp::BitAnd => "&=",
                    crate::parser::AugOp::BitOr => "|=",
                    crate::parser::AugOp::BitXor => "^=",
                    crate::parser::AugOp::ShiftLeft => "<<=",
                    crate::parser::AugOp::ShiftRight => ">>=",
                };

                if !var_type.is_numeric() {
                    self.context.add_error_with_location(
                        format!(
                            "Cannot use '{}' operator on non-numeric type '{}'",
                            op_name,
                            var_type.to_str()
                        ),
                        span.line,
                        span.column,
                        None,
                        None,
                    );
                } else if !expr_type.is_numeric() {
                    self.context.add_error_with_location(
                        format!(
                            "Cannot use '{}' operator with non-numeric type '{}'",
                            op_name,
                            expr_type.to_str()
                        ),
                        span.line,
                        span.column,
                        None,
                        None,
                    );
                }
            }
            Stmt::Return { expr, span } => {
                let expected_return = self.context.current_method_return.clone();

                // Type check the return expression (even if there's no expected return type)
                if let Some(e) = expr {
                    if let Some(expected) = &expected_return {
                        // Use expected type for type deduction
                        let expr_type = self.infer_expr_with_expected_type(e, &expected_return);
                        if !expr_type.is_assignable_to(expected) {
                            let (line, column) = Self::get_expr_span(e);
                            self.context.add_error_with_location(
                                format!(
                                    "Return type mismatch: expected {}, got {}",
                                    expected.to_str(),
                                    expr_type.to_str()
                                ),
                                line,
                                column,
                                None,
                                None,
                            );
                        }
                    } else {
                        // No expected return type (e.g., module-level return or void function)
                        // Still need to type-check the expression to catch errors like undeclared variables
                        self.infer_expr(e);
                    }
                } else if let Some(expected) = &expected_return {
                    if !matches!(expected, Type::Null | Type::Unknown) {
                        self.context.add_error_with_location(
                            format!(
                                "Expected return value of type {}, but no value returned",
                                expected.to_str()
                            ),
                            span.line,
                            span.column,
                            None,
                            None,
                        );
                    }
                }
            }
            Stmt::Expr(expr) => {
                self.infer_expr(expr);
            }
            Stmt::If { condition, then_branch, else_branch, .. } => {
                let cond_type = self.infer_expr(condition);
                if cond_type != Type::Bool && cond_type != Type::Unknown {
                    let (line, column) = Self::get_expr_span(condition);
                    self.context.add_error_with_location(
                        format!("Expected bool condition, got {}", cond_type.to_str()),
                        line,
                        column,
                        None,
                        None,
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
            Stmt::For { var_name, range, body, .. } => {
                let _range_type = self.infer_expr(range);
                // For now, assume ranges are integers
                self.context.add_variable(var_name, Type::Int, false);
                for stmt in body {
                    self.check_stmt(stmt);
                }
                self.context.variables.remove(var_name);
            }
            Stmt::While { condition, body, .. } => {
                let cond_type = self.infer_expr(condition);
                if cond_type != Type::Bool && cond_type != Type::Unknown {
                    let (line, column) = Self::get_expr_span(condition);
                    self.context.add_error_with_location(
                        format!("Expected bool condition for while, got {}", cond_type.to_str()),
                        line,
                        column,
                        None,
                        None,
                    );
                }
                for stmt in body {
                    self.check_stmt(stmt);
                }
            }
            Stmt::TryCatch { try_block, catch_var, catch_block, .. } => {
                for stmt in try_block {
                    self.check_stmt(stmt);
                }

                // Add catch variable (exception object) - currently unknown type
                self.context.add_variable(catch_var, Type::Unknown, false);

                for stmt in catch_block {
                    self.check_stmt(stmt);
                }

                self.context.variables.remove(catch_var);
            }
            Stmt::Throw { expr, .. } => {
                self.infer_expr(expr);
            }
            Stmt::Break(_) => {
                // Break statement - no type checking needed
            }
            Stmt::Continue(_) => {
                // Continue statement - no type checking needed
            }
        }
    }

    fn check_class(&mut self, class: &ClassDef) {
        let old_class = self.context.current_class.clone();
        self.context.current_class = Some(class.name.clone());

        // Check field default expressions
        for field in &class.fields {
            if let Some(default_expr) = &field.default {
                let field_type = Type::from_str(&field.type_name);
                let expr_type = self.infer_expr_with_expected_type(default_expr, &Some(field_type.clone()));

                if !expr_type.is_assignable_to(&field_type) {
                    let (line, column) = Self::get_expr_span(default_expr);
                    self.context.add_error_with_location(
                        format!(
                            "Type mismatch: cannot assign {} to field '{}' of type {}",
                            expr_type.to_str(),
                            field.name,
                            field_type.to_str()
                        ),
                        line,
                        column,
                        None,
                        None,
                    );
                }
            }
        }

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

        // Add type parameters as local type variables for generic functions
        // This allows type parameters like T to be recognized in the function body
        let mut added_type_params = Vec::new();
        for type_param in &func.type_params {
            // Add type parameter as a variable with TypeParameter type
            // This allows the type checker to recognize T as a valid type
            self.context.add_variable(type_param, Type::TypeParameter(type_param.clone()), false);
            added_type_params.push(type_param.clone());
        }

        // Add parameters as local variables
        let mut added_vars = Vec::new();
        for param in &func.params {
            let param_type = param.type_name.as_ref()
                .map(|t| Type::from_str(t))
                .unwrap_or(Type::Unknown);
            self.context.add_variable(&param.name, param_type.clone(), false);
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

        // Clean up type parameters
        for type_param in added_type_params {
            self.context.variables.remove(&type_param);
        }

        self.context.current_method_return = old_return;
    }

    fn check_method(&mut self, method: &Method, class_name: &str) {
        let old_return = self.context.current_method_return.clone();

        // Handle optional return types
        let return_type = method.return_type.as_ref().map(|t| {
            let mut ty = Type::from_str(t);
            // Resolve 'self' type to the actual class type
            if ty == Type::SelfType {
                ty = Type::Class(class_name.to_string());
            }
            if method.return_optional {
                Type::Optional(Box::new(ty))
            } else {
                ty
            }
        });

        self.context.current_method_return = return_type;

        // Add type parameters as local type variables for generic methods
        let mut added_type_params = Vec::new();
        for type_param in &method.type_params {
            self.context.add_variable(type_param, Type::TypeParameter(type_param.clone()), false);
            added_type_params.push(type_param.clone());
        }

        // Add parameters as local variables
        let mut added_vars = Vec::new();
        for param in &method.params {
            let param_type = param.type_name.as_ref()
                .map(|t| Type::from_str(t))
                .unwrap_or(Type::Unknown);
            self.context.add_variable(&param.name, param_type.clone(), false);
            added_vars.push(param.name.clone());
        }

        // Add 'self' variable
        self.context.add_variable("self", Type::Class(class_name.to_string()), false);

        // Check method body
        for stmt in &method.body {
            self.check_stmt(stmt);
        }

        // Clean up local variables
        for var in added_vars {
            self.context.variables.remove(&var);
        }
        // Clean up type parameters
        for type_param in added_type_params {
            self.context.variables.remove(&type_param);
        }
        self.context.variables.remove("self");

        self.context.current_method_return = old_return;
    }

    pub fn infer_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Literal(lit) => {
                match lit {
                    Literal::String(_, _) => Type::Str,
                    Literal::Int(_, _) => Type::Int,
                    Literal::Float(_, _) => Type::Float,
                    Literal::Bool(_, _) => Type::Bool,
                    Literal::Null(_) => Type::Null,
                }
            }
            Expr::Variable { name, span } => {
                if let Some(var_info) = self.context.get_variable(name) {
                    var_info.type_name.clone()
                } else if let Some(current_class) = &self.context.current_class {
                    if let Some(class_info) = self.context.get_class(current_class) {
                        if let Some(field_info) = class_info.fields.get(name) {
                            // Static fields can be accessed without self, instance fields require self
                            if field_info.is_static {
                                return field_info.type_name.clone();
                            } else {
                                // Error: accessing instance member without self
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
                    }
                    // Variable not found in class context - check if it's a class name (for static access)
                    if self.context.get_class(name).is_some() {
                        // Class name reference - will be handled in Get expression
                        return Type::Class(name.clone());
                    }
                    // Variable not found - report as undeclared
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
                } else if self.context.get_class(name).is_some() {
                    // Class name reference (for static access) - will be handled in Get expression
                    Type::Class(name.clone())
                } else {
                    // Variable not found in global/function context - check if it's a module alias
                    let mut is_module_alias = false;
                    for import_entry in &self.context.imports {
                        if let Some(last_dot) = import_entry.module_path.rfind('.') {
                            let import_alias = &import_entry.module_path[last_dot + 1..];
                            if import_alias == name {
                                is_module_alias = true;
                                break;
                            }
                        } else if import_entry.module_path.as_str() == name {
                            is_module_alias = true;
                            break;
                        }
                    }

                    if is_module_alias {
                        return Type::Unknown;
                    }

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
            Expr::Binary { left, op, right, span } => {
                let left_type = self.infer_expr(left);
                let right_type = self.infer_expr(right);

                // Type checking for binary operations
                match op {
                    crate::parser::BinaryOp::Equal | crate::parser::BinaryOp::NotEqual => {
                        // Equality can be checked between any types, but they should match
                        // Special case: optional types (T?) can be compared with null
                        let is_valid_comparison = left_type == right_type
                            || left_type == Type::Unknown
                            || right_type == Type::Unknown
                            || matches!(&left_type, Type::Optional(_)) && right_type == Type::Null
                            || matches!(&right_type, Type::Optional(_)) && left_type == Type::Null;
                        
                        if !is_valid_comparison {
                            self.context.add_error_with_location(
                                format!(
                                    "Cannot compare {} with {} using equality operator",
                                    left_type.to_str(),
                                    right_type.to_str()
                                ),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        Type::Bool
                    }
                    crate::parser::BinaryOp::And | crate::parser::BinaryOp::Or => {
                        if left_type != Type::Bool && left_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Expected bool for logical operator, got {}", left_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        if right_type != Type::Bool && right_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Expected bool for logical operator, got {}", right_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        Type::Bool
                    }
                    crate::parser::BinaryOp::Add | crate::parser::BinaryOp::Subtract |
                    crate::parser::BinaryOp::Multiply | crate::parser::BinaryOp::Divide |
                    crate::parser::BinaryOp::Modulo => {
                        if op == &crate::parser::BinaryOp::Add && (left_type == Type::Str || right_type == Type::Str) {
                            return Type::Str;
                        }

                        // Arithmetic operations require numeric types
                        if !is_numeric_type(&left_type) && left_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Expected numeric type for arithmetic operation, got {}", left_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        if !is_numeric_type(&right_type) && right_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Expected numeric type for arithmetic operation, got {}", right_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        // Result type is the more precise type (float > int)
                        if left_type == Type::Float || right_type == Type::Float {
                            Type::Float
                        } else if left_type == right_type {
                            // If both operands have the same specific integer type, preserve it
                            match left_type {
                                Type::Int8 | Type::UInt8 | Type::Int16 | Type::UInt16 |
                                Type::Int32 | Type::UInt32 | Type::Int64 | Type::UInt64 => left_type.clone(),
                                _ => Type::Int,
                            }
                        } else {
                            Type::Int
                        }
                    }
                    crate::parser::BinaryOp::Pow => {
                        // Power operation requires numeric types and returns float
                        if !is_numeric_type(&left_type) && left_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Expected numeric type for power operation, got {}", left_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        if !is_numeric_type(&right_type) && right_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Expected numeric type for power operation, got {}", right_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        Type::Float
                    }
                    crate::parser::BinaryOp::Greater | crate::parser::BinaryOp::Less |
                    crate::parser::BinaryOp::GreaterEqual | crate::parser::BinaryOp::LessEqual => {
                        // Comparison operations require numeric types and return bool
                        if !is_numeric_type(&left_type) && left_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Expected numeric type for comparison, got {}", left_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        if !is_numeric_type(&right_type) && right_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Expected numeric type for comparison, got {}", right_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        Type::Bool
                    }
                    crate::parser::BinaryOp::BitAnd | crate::parser::BinaryOp::BitOr |
                    crate::parser::BinaryOp::BitXor | crate::parser::BinaryOp::ShiftLeft |
                    crate::parser::BinaryOp::ShiftRight => {
                        // Bitwise operations require integer types
                        if !is_integer_type(&left_type) && left_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Expected integer type for bitwise operation, got {}", left_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        if !is_integer_type(&right_type) && right_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Expected integer type for bitwise operation, got {}", right_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        // For shift operators, the right operand should be an integer (shift amount)
                        // Result type is the type of the left operand (for shifts) or the more specific integer type
                        match op {
                            crate::parser::BinaryOp::ShiftLeft | crate::parser::BinaryOp::ShiftRight => {
                                // Shift operators preserve the left operand's integer type
                                if let Type::Int8 | Type::UInt8 | Type::Int16 | Type::UInt16 |
                                   Type::Int32 | Type::UInt32 | Type::Int64 | Type::UInt64 = &left_type {
                                    left_type.clone()
                                } else {
                                    Type::Int
                                }
                            }
                            _ => {
                                // BitAnd, BitOr, BitXor: result is the more specific integer type
                                if left_type == right_type {
                                    match left_type {
                                        Type::Int8 | Type::UInt8 | Type::Int16 | Type::UInt16 |
                                        Type::Int32 | Type::UInt32 | Type::Int64 | Type::UInt64 => left_type.clone(),
                                        _ => Type::Int,
                                    }
                                } else {
                                    Type::Int
                                }
                            }
                        }
                    }
                }
            }
            Expr::Unary { op, expr, span } => {
                let inner_type = self.infer_expr(expr);
                match op {
                    crate::parser::UnaryOp::Not => {
                        if inner_type != Type::Bool && inner_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Expected bool for ! operator, got {}", inner_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        Type::Bool
                    }
                    crate::parser::UnaryOp::BitNot => {
                        // Bitwise NOT requires integer types
                        if !is_integer_type(&inner_type) && inner_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Expected integer type for bitwise NOT, got {}", inner_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        // Result type is the same as operand type
                        if let Type::Int8 | Type::UInt8 | Type::Int16 | Type::UInt16 |
                           Type::Int32 | Type::UInt32 | Type::Int64 | Type::UInt64 = &inner_type {
                            inner_type.clone()
                        } else {
                            Type::Int
                        }
                    }
                    crate::parser::UnaryOp::Negate => {
                        // Negation works on numeric types and returns the same type
                        if inner_type != Type::Int && inner_type != Type::Float && !inner_type.is_numeric() && inner_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Expected numeric type for unary minus, got {}", inner_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        inner_type
                    }
                    crate::parser::UnaryOp::PrefixIncrement | crate::parser::UnaryOp::PostfixIncrement => {
                        // Increment operator works on numeric types and returns the original type
                        if !inner_type.is_numeric() && inner_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Expected numeric type for ++ operator, got {}", inner_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        inner_type
                    }
                    crate::parser::UnaryOp::PrefixDecrement | crate::parser::UnaryOp::PostfixDecrement | crate::parser::UnaryOp::Decrement => {
                        // Decrement operator works on numeric types and returns the original type
                        if !inner_type.is_numeric() && inner_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Expected numeric type for -- operator, got {}", inner_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                        inner_type
                    }
                }
            }
            Expr::Call { callee, args, span } => {
                // Extract span from callee for accurate error reporting
                let (func_name, func_span) = if let Expr::Variable { name: func_name, span: func_span } = callee.as_ref() {
                    (func_name.as_str(), func_span)
                } else {
                    ("", span)
                };

                if !func_name.is_empty() {
                    // Check if this is a variable with a function type (e.g., lambda variable)
                    // or a variable of type 'any' (which can be called dynamically)
                    let var_info_and_type = self.context.get_variable(func_name)
                        .map(|var_info| (var_info.type_name.clone(), var_info.name.clone()));
                    
                    if let Some((var_type, _var_name)) = var_info_and_type {
                        if let Type::Function(param_types, return_type) = &var_type {
                            // This is a variable of function type - check the call
                            let arg_types: Vec<Type> = args.iter()
                                .map(|arg| self.infer_expr(arg))
                                .collect();

                            // Check argument count
                            if param_types.len() != arg_types.len() {
                                self.context.add_error_with_location(
                                    format!("Function expects {} argument(s), got {}", param_types.len(), arg_types.len()),
                                    span.line, span.column, None, None
                                );
                            } else {
                                // Check argument types
                                for (i, (param_type, arg_type)) in param_types.iter().zip(arg_types.iter()).enumerate() {
                                    if !arg_type.is_assignable_to(param_type) && *param_type != Type::Unknown && *arg_type != Type::Unknown {
                                        self.context.add_error_with_location(
                                            format!("Argument {}: expected {}, got {}", i + 1, param_type.to_str(), arg_type.to_str()),
                                            span.line, span.column, None, None
                                        );
                                    }
                                }
                            }

                            return return_type.as_ref().clone();
                        } else if var_type == Type::Any {
                            // Variable of type 'any' can be called dynamically (no type checking)
                            // Infer argument types but don't check them
                            for arg in args {
                                self.infer_expr(arg);
                            }
                            return Type::Any;
                        }
                    }

                    // Check if this is a generic function call like identity<T>(args)
                    let (base_func_name, type_args_str) = Self::parse_generic_function_name(func_name);

                    if !type_args_str.is_empty() {
                        // This is a generic function call with explicit type arguments
                        // First, resolve the base function
                        let arg_types: Vec<Type> = args.iter()
                            .map(|arg| self.infer_expr(arg))
                            .collect();

                        let func_sig_opt = self.context.resolve_function_call(base_func_name, &arg_types);
                        if let Some(sig) = func_sig_opt.cloned() {
                            if !sig.type_params.is_empty() {
                                // This is a generic function - substitute type parameters
                                let type_args: Vec<Type> = type_args_str.iter().map(|t| Type::from_str(t)).collect();

                                // Validate type argument count
                                if type_args.len() != sig.type_params.len() {
                                    self.context.add_error_with_location(
                                        format!(
                                            "Generic function '{}' expects {} type argument(s), got {}",
                                            base_func_name,
                                            sig.type_params.len(),
                                            type_args.len()
                                        ),
                                        func_span.line, func_span.column, None, None
                                    );
                                }

                                // Substitute type parameters in signature
                                let substituted_params: Vec<ParamSignature> = sig.params.iter().map(|p| {
                                    ParamSignature {
                                        name: p.name.clone(),
                                        type_name: p.type_name.as_ref().map(|t|
                                            self.context.substitute_type_params(t, &type_args, &sig.type_params)
                                        ),
                                        default: false,
                                    }
                                }).collect();

                                let substituted_return = sig.return_type.as_ref().map(|t|
                                    self.context.substitute_type_params(t, &type_args, &sig.type_params)
                                );

                                // Create substituted signature for checking
                                let substituted_sig = FunctionSignature {
                                    name: sig.name.clone(),
                                    params: substituted_params,
                                    return_type: substituted_return,
                                    return_optional: sig.return_optional,
                                    is_method: sig.is_method,
                                    is_native: sig.is_native,
                                    private: sig.private,
                                    type_params: sig.type_params.clone(),
                                    mangled_name: sig.mangled_name.clone(),
                                };

                                self.check_function_call(&substituted_sig, args, func_name, func_span.line, func_span.column);

                                return substituted_sig.return_type.clone().unwrap_or(Type::Unknown);
                            }
                            // Not a generic function, fall through to regular function call
                        }
                        // Function not found, fall through to class instantiation check
                    }

                    // Regular function call - use overload resolution
                    // First, infer argument types for overload resolution
                    let arg_types: Vec<Type> = args.iter()
                        .map(|arg| self.infer_expr(arg))
                        .collect();

                    // Use resolve_function_call for proper overload resolution
                    let func_sig = self.context.resolve_function_call(func_name, &arg_types);
                    if let Some(sig) = func_sig.cloned() {
                        self.check_function_call(&sig, args, func_name, func_span.line, func_span.column);
                        sig.return_type.clone().unwrap_or(Type::Unknown)
                    } else {
                        // Function not found or no matching overload
                        // Check if there are any overloads available for this function name
                        let overloads = self.context.get_available_overloads(func_name, &arg_types);
                        if !overloads.is_empty() {
                            // Function exists but no matching overload - report detailed error
                            let error_msg = self.context.format_no_matching_overload_error(func_name, &arg_types, func_span.line, func_span.column);
                            self.context.add_error_with_location(error_msg, func_span.line, func_span.column, None, None);
                            return Type::Unknown;
                        }
                        
                        // Check if it's a class instantiation (possibly generic)
                        // Parse func_name as a type to handle generic instances like ClassName<T>
                        let func_name_type = Type::from_str(func_name);

                        let (base_class_name, full_return_type) = match &func_name_type {
                            Type::GenericInstance(base_name, type_args) => {
                                // This is a generic class instantiation like ClassName<T1, T2>
                                // Look up the base class
                                if let Some(class_info) = self.context.get_class(base_name) {
                                    // Validate type arguments against class type parameters
                                    if !class_info.type_params.is_empty() {
                                        if type_args.len() != class_info.type_params.len() {
                                            self.context.add_error_with_location(
                                                format!(
                                                    "Generic class '{}' expects {} type argument(s), got {}",
                                                    base_name,
                                                    class_info.type_params.len(),
                                                    type_args.len()
                                                ),
                                                func_span.line, func_span.column, None, None
                                            );
                                        }
                                    }
                                    // Return type is the generic instance
                                    (base_name.clone(), func_name_type.clone())
                                } else {
                                    // Base class not found
                                    self.context.add_error_with_location(
                                        format!("Undefined class: '{}'", base_name),
                                        func_span.line, func_span.column, None, None
                                    );
                                    return Type::Unknown;
                                }
                            }
                            Type::Class(base_name) => {
                                // Non-generic class instantiation
                                if let Some(_class_info) = self.context.get_class(base_name) {
                                    (base_name.clone(), Type::Class(base_name.clone()))
                                } else {
                                    // Not a class, check if it's a type alias or enum
                                    if let Some(_alias_info) = self.context.get_type_alias(func_name) {
                                        // Type alias instantiation - treat as the aliased type
                                        return func_name_type.clone();
                                    }
                                    if let Some(_enum_info) = self.context.get_enum(func_name) {
                                        // Enum instantiation
                                        return Type::Enum(func_name.to_string());
                                    }
                                    // Not found at all
                                    self.context.add_error_with_location(
                                        format!("Undefined function: '{}'", func_name),
                                        func_span.line, func_span.column, None, None
                                    );
                                    return Type::Unknown;
                                }
                            }
                            _ => {
                                // Not a valid class type
                                self.context.add_error_with_location(
                                    format!("Undefined function: '{}'", func_name),
                                    func_span.line, func_span.column, None, None
                                );
                                return Type::Unknown;
                            }
                        };

                        // It's a class instantiation - check constructor args
                        // For generic classes, we need to substitute type parameters in the constructor signature
                        if let Type::GenericInstance(base_name, type_args) = &func_name_type {
                            // Look up the base class and get its type parameters
                            if let Some(class_info) = self.context.get_class(base_name) {
                                let type_params = class_info.type_params.clone();
                                
                                // Try to find a constructor and substitute type parameters
                                let mut found_matching_ctor = false;
                                
                                if let Some(overloads) = class_info.method_overloads.get("constructor") {
                                    for mangled in overloads {
                                        if let Some(sig) = class_info.methods.get(mangled) {
                                            // Substitute type parameters in the constructor signature
                                            let substituted_params: Vec<crate::types::ParamSignature> = sig.params.iter().map(|p| {
                                                let substituted_type = p.type_name.as_ref().map(|t| {
                                                    self.context.substitute_type_params(t, type_args, &type_params)
                                                });
                                                crate::types::ParamSignature {
                                                    name: p.name.clone(),
                                                    type_name: substituted_type,
                                                    default: false,
                                                }
                                            }).collect();
                                            
                                            // Check if argument count matches
                                            let substituted_param_types: Vec<Type> = substituted_params.iter()
                                                .filter_map(|p| p.type_name.clone())
                                                .collect();
                                            
                                            if substituted_param_types.len() != arg_types.len() {
                                                continue; // Wrong number of arguments
                                            }
                                            
                                            // Check if all argument types match
                                            let mut all_match = true;
                                            for (expected, actual) in substituted_param_types.iter().zip(arg_types.iter()) {
                                                // Types match if they're equal, or one is assignable to the other
                                                let types_match = expected == actual 
                                                    || expected.is_assignable_to(actual)
                                                    || actual.is_assignable_to(expected)
                                                    || *expected == Type::Unknown
                                                    || *actual == Type::Unknown;
                                                if !types_match {
                                                    all_match = false;
                                                    break;
                                                }
                                            }
                                            
                                            if all_match {
                                                found_matching_ctor = true;
                                                // Check the method call with substituted signature
                                                let substituted_sig = MethodSignature {
                                                    name: sig.name.clone(),
                                                    params: substituted_params,
                                                    return_type: sig.return_type.as_ref().map(|t| {
                                                        self.context.substitute_type_params(t, type_args, &type_params)
                                                    }),
                                                    return_optional: sig.return_optional,
                                                    private: sig.private,
                                                    is_native: sig.is_native,
                                                    is_static: sig.is_static,
                                                    type_params: sig.type_params.clone(),
                                                    mangled_name: sig.mangled_name.clone(),
                                                };
                                                self.check_method_call(&substituted_sig, args, "constructor", &base_class_name, span.line, span.column);
                                                break;
                                            }
                                        }
                                    }
                                }

                                if !found_matching_ctor && !args.is_empty() {
                                    self.context.add_error_with_location(
                                        format!("Class '{}' does not accept constructor arguments", base_class_name),
                                        span.line, span.column, None, None
                                    );
                                }
                            }
                        } else {
                            // Non-generic class - use normal resolution
                            let ctor_sig = self.context.resolve_method_call(&base_class_name, "constructor", &arg_types);
                            if let Some(sig) = ctor_sig.cloned() {
                                self.check_method_call(&sig, args, "constructor", &base_class_name, span.line, span.column);
                            } else if !args.is_empty() {
                                self.context.add_error_with_location(
                                    format!("Class '{}' does not accept constructor arguments", base_class_name),
                                    span.line, span.column, None, None
                                );
                            }
                        }
                        
                        // Return type is the full type (generic instance or plain class)
                        full_return_type
                    }
                } else if let Expr::Get { object, name, span: method_span } = callee.as_ref() {
                    // Could be method call OR module.function() call OR nested class instantiation
                    let object_type = self.infer_expr(object);

                    // First, check if this is a nested class instantiation (e.g., Outer.Inner(args))
                    if let Expr::Variable { name: obj_name, .. } = object.as_ref() {
                        if let Some(_outer_class_info) = self.context.get_class(obj_name) {
                            let nested_class_name = format!("{}.{}", obj_name, name);
                            if let Some(_nested_class_info) = self.context.get_class(&nested_class_name) {
                                // This is a nested class instantiation
                                let arg_types: Vec<Type> = args.iter()
                                    .map(|arg| self.infer_expr(arg))
                                    .collect();
                                let ctor_sig = self.context.resolve_method_call(&nested_class_name, "constructor", &arg_types);
                                if let Some(sig) = ctor_sig.cloned() {
                                    self.check_method_call(&sig, args, "constructor", &nested_class_name, method_span.line, method_span.column);
                                } else if !args.is_empty() {
                                    self.context.add_error_with_location(
                                        format!("Class '{}' does not accept constructor arguments", nested_class_name),
                                        method_span.line, method_span.column, None, None
                                    );
                                }
                                return Type::Class(nested_class_name);
                            }
                        }
                    }

                    let effective_class = match object_type {
                        Type::Class(ref name) => Some(name.clone()),
                        Type::Str => Some("str".to_string()),
                        Type::Array(_) => Some("Array".to_string()),
                        _ => None,
                    };

                    if let Some(class_name) = effective_class {
                        // This is a method call on a class instance or built-in type
                        let arg_types: Vec<Type> = args.iter()
                            .map(|arg| self.infer_expr(arg))
                            .collect();
                        let method_sig = self.context.resolve_method_call(&class_name, name, &arg_types);

                        if let Some(sig) = method_sig.cloned() {
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
                                self.context.add_error_with_location(err, method_span.line, method_span.column, None, None);
                            }

                            self.check_method_call(&sig, args, name, &class_name, method_span.line, method_span.column);
                            return sig.return_type.clone().unwrap_or(Type::Unknown);
                        } else if !matches!(object_type, Type::Class(_)) {
                            // If it's a built-in type and method not found, don't fallback to module lookup
                            self.context.add_error_with_location(
                                format!("Method '{}' not found on type '{}'", name, object_type.to_str()),
                                method_span.line, method_span.column, None, None
                            );
                            return Type::Unknown;
                        }
                    }

                    if let Expr::Variable { name: module_name, .. } = object.as_ref() {
                        // This is module.function() call - look up qualified name
                        // Try to resolve as a qualified module function
                        let func_sig = self.context.resolve_qualified_function(module_name, name);
                        if let Some(sig) = func_sig.cloned() {
                            let _arg_types: Vec<Type> = args.iter()
                                .map(|arg| self.infer_expr(arg))
                                .collect();
                            self.check_function_call(&sig, args, &sig.name, method_span.line, method_span.column);
                            return sig.return_type.clone().unwrap_or(Type::Unknown);
                        } else if let Some(qualified_class_name) = self.context.resolve_qualified_class(module_name, name) {
                            // It's a qualified class instantiation
                            let arg_types: Vec<Type> = args.iter()
                                .map(|arg| self.infer_expr(arg))
                                .collect();
                            let ctor_sig = self.context.resolve_method_call(&qualified_class_name, "constructor", &arg_types);
                            if let Some(sig) = ctor_sig.cloned() {
                                self.check_method_call(&sig, args, "constructor", &qualified_class_name, method_span.line, method_span.column);
                            } else if !args.is_empty() {
                                self.context.add_error_with_location(
                                    format!("Class '{}' does not accept constructor arguments", qualified_class_name),
                                    method_span.line, method_span.column, None, None
                                );
                            }
                            Type::Class(qualified_class_name)
                        } else {
                            self.context.add_error_with_location(
                                format!("Undefined function or class: '{}.{}'", module_name, name),
                                method_span.line, method_span.column, None, None
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
            Expr::Get { object, name, span } => {
                // Check for static member access: ClassName.member
                if let Expr::Variable { name: obj_name, span: _ } = object.as_ref() {
                    // Check if obj_name is a class name
                    if let Some(class_info) = self.context.get_class(obj_name) {
                        // First, check if this is a nested class access (e.g., Outer.Inner)
                        let nested_class_name = format!("{}.{}", obj_name, name);
                        if let Some(nested_class_info) = self.context.get_class(&nested_class_name) {
                            // Check visibility
                            if nested_class_info.private {
                                if let Some(current) = &self.context.current_class {
                                    if current != obj_name {
                                        self.context.add_error_with_location(
                                            format!("Nested class '{}' is private and cannot be accessed from class '{}'", name, current),
                                            span.line, span.column, None, None
                                        );
                                        return Type::Unknown;
                                    }
                                } else {
                                    self.context.add_error_with_location(
                                        format!("Nested class '{}' is private and cannot be accessed from global scope", name),
                                        span.line, span.column, None, None
                                    );
                                    return Type::Unknown;
                                }
                            }
                            // Return the nested class type
                            return Type::Class(nested_class_name);
                        }
                        
                        // This is static member access
                        if let Some(field_info) = class_info.fields.get(name) {
                            if field_info.is_static {
                                // Check visibility
                                let mut visibility_error = None;
                                if field_info.private {
                                    if let Some(current) = &self.context.current_class {
                                        if current != obj_name {
                                            visibility_error = Some(format!("Static field '{}' on class '{}' is private and cannot be accessed from class '{}'", name, obj_name, current));
                                        }
                                    } else {
                                        visibility_error = Some(format!("Static field '{}' on class '{}' is private and cannot be accessed from global scope", name, obj_name));
                                    }
                                }

                                let type_name = field_info.type_name.clone();

                                if let Some(err) = visibility_error {
                                    self.context.add_error_with_location(err, span.line, span.column, None, None);
                                }

                                return type_name;
                            } else {
                                self.context.add_error_with_location(
                                    format!("Cannot access instance field '{}' on class '{}' without an instance. Use 'self.{}' or make the field static.", name, obj_name, name),
                                    span.line, span.column, None, None
                                );
                                return Type::Unknown;
                            }
                        } else if let Some(method_sig) = class_info.methods.get(&format!("{}()", name)) {
                            // Static method reference (not call) - return function type
                            if method_sig.is_static {
                                let param_types: Vec<Type> = method_sig.params.iter()
                                    .filter_map(|p| p.type_name.clone())
                                    .collect();
                                let return_type = method_sig.return_type.clone().unwrap_or(Type::Unknown);
                                return Type::Function(param_types, Box::new(return_type));
                            }
                        }
                        // Member not found as static - check if it's an instance member
                        if class_info.fields.contains_key(name) ||
                           class_info.methods.values().any(|m| m.name == *name) {
                            self.context.add_error_with_location(
                                format!("Cannot access instance member '{}' on class '{}' without an instance. Use 'self.{}' or make the member static.", name, obj_name, name),
                                span.line, span.column, None, None
                            );
                            return Type::Unknown;
                        }
                        self.context.add_error_with_location(
                            format!("Member '{}' not found on class '{}'", name, obj_name),
                            span.line, span.column, None, None
                        );
                        return Type::Unknown;
                    }
                }

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

                            // Static fields must be accessed through the class name, not an instance
                            if field_info.is_static {
                                self.context.add_error_with_location(
                                    format!("Static field '{}' on class '{}' must be accessed through the class name, not an instance. Use '{}.{}' instead.", name, class_name, class_name, name),
                                    span.line, span.column, None, None
                                );
                            }

                            if let Some(err) = visibility_error {
                                self.context.add_error_with_location(err, span.line, span.column, None, None);
                            }

                            type_name
                        } else {
                            self.context.add_error_with_location(
                                format!("Field '{}' not found on class '{}'", name, class_name),
                                span.line, span.column, None, None
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
                            self.context.add_error_with_location(
                                format!("Variant '{}' not found on enum '{}'", name, enum_name),
                                span.line, span.column, None, None
                            );
                            Type::Unknown
                        }
                    } else {
                        Type::Unknown
                    }
                } else if let Expr::Variable { name: module_name, .. } = object.as_ref() {
                    // Module-level variable access (e.g., math.PI)
                    // Check if module_name is an imported module alias
                    if let Some(var_info) = self.context.resolve_qualified_variable(module_name, name) {
                        return var_info.type_name.clone();
                    }

                    // Also check for enums
                    let direct_name = format!("{}.{}", module_name, name);
                    if let Some(_enum_info) = self.context.get_enum(&direct_name) {
                        return Type::Enum(direct_name);
                    }

                    for import_entry in &self.context.imports {
                        let qualified_enum = format!("{}.{}.{}", import_entry.module_path, module_name, name);
                        if self.context.get_enum(&qualified_enum).is_some() {
                            return Type::Enum(qualified_enum);
                        }
                    }

                    // Check for classes
                    if let Some(qualified_class) = self.context.resolve_qualified_class(module_name, name) {
                        return Type::Class(qualified_class);
                    }

                    Type::Unknown
                } else {
                    Type::Unknown
                }
            }
            Expr::Set { object, name, value, span } => {
                let object_type = self.infer_expr(object);
                let _value_type = self.infer_expr(value);

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

                        // Use expected type for type deduction
                        let expected_field_type = &field.type_name;
                        let deduced_type = self.infer_expr_with_expected_type(value, &Some(expected_field_type.clone()));
                        
                        if !deduced_type.is_assignable_to(expected_field_type) {
                            self.context.add_error_with_location(
                                format!(
                                    "Cannot assign {} to field '{}' of type {}",
                                    deduced_type.to_str(),
                                    name,
                                    expected_field_type.to_str()
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
            Expr::Range { start, end, span } => {
                let start_type = self.infer_expr(start);
                let end_type = self.infer_expr(end);
                if start_type != Type::Int && start_type != Type::Unknown {
                    self.context.add_error_with_location(format!("Range start must be an integer, got {}", start_type.to_str()), span.line, span.column, None, None);
                }
                if end_type != Type::Int && end_type != Type::Unknown {
                    self.context.add_error_with_location(format!("Range end must be an integer, got {}", end_type.to_str()), span.line, span.column, None, None);
                }
                Type::Int
            }
            Expr::Cast { expr, target_type, span } => {
                // Type check the inner expression
                let inner_type = self.infer_expr(expr);

                // Validate cast is reasonable (allow most casts, warn about nonsensical ones)
                match target_type {
                    CastType::Int | CastType::Int8 | CastType::UInt8 | CastType::Int16 | CastType::UInt16 |
                    CastType::Int32 | CastType::UInt32 | CastType::Int64 | CastType::UInt64 => {
                        // int() can accept: float, str, bool, int, and other numeric types
                        if !inner_type.is_numeric() &&
                           inner_type != Type::Str &&
                           inner_type != Type::Bool &&
                           inner_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Cannot cast {} to integer type", inner_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                    }
                    CastType::Float | CastType::Float32 | CastType::Float64 => {
                        // float() can accept: int, str, bool, float, and other numeric types
                        if !inner_type.is_numeric() &&
                           inner_type != Type::Str &&
                           inner_type != Type::Bool &&
                           inner_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Cannot cast {} to float type", inner_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }
                    }
                    CastType::Str => {
                        // str() can accept any type
                    }
                    CastType::Bool => {
                        // bool() can accept: int, float, str, bool
                        if !inner_type.is_numeric() &&
                           inner_type != Type::Str &&
                           inner_type != Type::Bool &&
                           inner_type != Type::Unknown {
                            self.context.add_error_with_location(
                                format!("Cannot cast {} to bool", inner_type.to_str()),
                                span.line,
                                span.column,
                                None,
                                None,
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
                    CastType::Int8 => Type::Int8,
                    CastType::UInt8 => Type::UInt8,
                    CastType::Int16 => Type::Int16,
                    CastType::UInt16 => Type::UInt16,
                    CastType::Int32 => Type::Int32,
                    CastType::UInt32 => Type::UInt32,
                    CastType::Int64 => Type::Int64,
                    CastType::UInt64 => Type::UInt64,
                    CastType::Float32 => Type::Float32,
                    CastType::Float64 => Type::Float64,
                }
            }
            Expr::Array { elements, .. } => {
                let mut element_type = Type::Unknown;
                if !elements.is_empty() {
                    element_type = self.infer_expr(&elements[0]);
                    for el in elements.iter().skip(1) {
                        let ty = self.infer_expr(el);
                        if !ty.is_assignable_to(&element_type) {
                            // Elements should be compatible
                        }
                    }
                }
                Type::Array(Box::new(element_type))
            }
            Expr::Index { object, index, span } => {
                let object_type = self.infer_expr(object);
                let index_type = self.infer_expr(index);

                if index_type != Type::Int && index_type != Type::Unknown {
                    self.context.add_error_with_location(
                        format!("Array index must be an integer, got {}", index_type.to_str()),
                        span.line,
                        span.column,
                        None,
                        None,
                    );
                }

                match object_type {
                    Type::Array(inner) => *inner,
                    Type::Str => Type::Str,
                    Type::Unknown => Type::Unknown,
                    _ => {
                        self.context.add_error_with_location(
                            format!("Type {} does not support indexing", object_type.to_str()),
                            span.line,
                            span.column,
                            None,
                            None,
                        );
                        Type::Unknown
                    }
                }
            }
            Expr::ObjectLiteral { fields, .. } => {
                // Object literal without expected type - return Unknown
                // Type will be inferred from context when called with infer_expr_with_expected_type
                for field in fields {
                    self.infer_expr(&field.value);
                }
                Type::Unknown
            }
            Expr::Lambda { params, return_type, body, .. } => {
                // Infer lambda type: (param_types) -> return_type
                let param_types: Vec<Type> = params.iter()
                    .map(|p| p.type_name.as_ref()
                        .map(|t| Type::from_str(t))
                        .unwrap_or(Type::Unknown))
                    .collect();

                let ret_type = return_type.as_ref()
                    .map(|t| Type::from_str(t))
                    .unwrap_or(Type::Null);

                // Note: 'self' type in lambda return is not resolved here since lambdas
                // don't have a class context. It would remain as SelfType which would
                // only match with another SelfType (unlikely to be useful in lambdas).

                // Type check the lambda body
                // Add parameters as local variables
                let mut added_vars = Vec::new();
                for param in params {
                    let param_type = param.type_name.as_ref()
                        .map(|t| Type::from_str(t))
                        .unwrap_or(Type::Unknown);
                    self.context.add_variable(&param.name, param_type, false);
                    added_vars.push(param.name.clone());
                }

                // Store expected return type
                let old_return = self.context.current_method_return.clone();
                self.context.current_method_return = Some(ret_type.clone());

                // Check body
                for stmt in body {
                    self.check_stmt(stmt);
                }

                // Clean up
                for var in added_vars {
                    self.context.variables.remove(&var);
                }
                self.context.current_method_return = old_return;

                Type::Function(param_types, Box::new(ret_type))
            }
        }
    }

    /// Infer expression type with an expected type hint (for type deduction)
    fn infer_expr_with_expected_type(&mut self, expr: &Expr, expected_type: &Option<Type>) -> Type {
        match expr {
            Expr::Literal(lit) => {
                match lit {
                    Literal::Int(value, span) => {
                        // Check if the expected type is a specific integer type and validate range
                        if let Some(expected) = expected_type {
                            match expected {
                                Type::Int8 => {
                                    if *value < -128 || *value > 127 {
                                        self.context.add_error_with_location(
                                            format!("Value {} is out of range for int8 (-128 to 127)", value),
                                            span.line,
                                            span.column,
                                            None,
                                            None,
                                        );
                                    }
                                    return Type::Int8;
                                }
                                Type::UInt8 => {
                                    if *value < 0 || *value > 255 {
                                        self.context.add_error_with_location(
                                            format!("Value {} is out of range for uint8 (0 to 255)", value),
                                            span.line,
                                            span.column,
                                            None,
                                            None,
                                        );
                                    }
                                    return Type::UInt8;
                                }
                                Type::Int16 => {
                                    if *value < -32768 || *value > 32767 {
                                        self.context.add_error_with_location(
                                            format!("Value {} is out of range for int16 (-32768 to 32767)", value),
                                            span.line,
                                            span.column,
                                            None,
                                            None,
                                        );
                                    }
                                    return Type::Int16;
                                }
                                Type::UInt16 => {
                                    if *value < 0 || *value > 65535 {
                                        self.context.add_error_with_location(
                                            format!("Value {} is out of range for uint16 (0 to 65535)", value),
                                            span.line,
                                            span.column,
                                            None,
                                            None,
                                        );
                                    }
                                    return Type::UInt16;
                                }
                                Type::Int32 => {
                                    if *value < -2147483648 || *value > 2147483647 {
                                        self.context.add_error_with_location(
                                            format!("Value {} is out of range for int32", value),
                                            span.line,
                                            span.column,
                                            None,
                                            None,
                                        );
                                    }
                                    return Type::Int32;
                                }
                                Type::UInt32 => {
                                    if *value < 0 || *value > 4294967295 {
                                        self.context.add_error_with_location(
                                            format!("Value {} is out of range for uint32 (0 to 4294967295)", value),
                                            span.line,
                                            span.column,
                                            None,
                                            None,
                                        );
                                    }
                                    return Type::UInt32;
                                }
                                Type::Int64 => {
                                    return Type::Int64;
                                }
                                Type::UInt64 => {
                                    if *value < 0 {
                                        self.context.add_error_with_location(
                                            format!("Value {} is out of range for uint64 (must be non-negative)", value),
                                            span.line,
                                            span.column,
                                            None,
                                            None,
                                        );
                                    }
                                    return Type::UInt64;
                                }
                                _ => {}
                            }
                        }
                        Type::Int
                    }
                    Literal::Float(_value, _) => {
                        if let Some(expected) = expected_type {
                            match expected {
                                Type::Float32 => return Type::Float32,
                                Type::Float64 => return Type::Float64,
                                _ => {}
                            }
                        }
                        Type::Float
                    }
                    Literal::String(_, _) => Type::Str,
                    Literal::Bool(_, _) => Type::Bool,
                    Literal::Null(_) => Type::Null,
                }
            }
            Expr::Array { elements, span } => {
                // Try to infer array element type from expected type
                if let Some(expected) = expected_type {
                    if let Type::Array(expected_element_type) = expected {
                        // Use expected element type for type checking
                        for el in elements {
                            self.infer_expr_with_expected_type(el, &Some(*expected_element_type.clone()));
                        }
                        // Store the inferred type for code generation
                        if let Type::TypeParameter(_param_name) = expected_element_type.as_ref() {
                            // For type parameters, store the parameterized array type
                            self.context.expr_types.insert((span.line, span.column), expected.to_str());
                        }
                        return expected.clone();
                    }
                }
                // No expected type - infer from elements
                let mut element_type = Type::Unknown;
                if !elements.is_empty() {
                    element_type = self.infer_expr(&elements[0]);
                    for el in elements.iter().skip(1) {
                        let ty = self.infer_expr(el);
                        if !ty.is_assignable_to(&element_type) {
                            // Elements should be compatible
                        }
                    }
                }
                Type::Array(Box::new(element_type))
            }
            Expr::ObjectLiteral { fields, span, .. } => {
                // Try to infer the class type from the expected type
                if let Some(expected) = expected_type {
                    if let Type::Class(class_name) = expected {
                        // Verify the object literal matches the class structure
                        // First, collect field info to avoid borrow issues
                        let class_info_opt = self.context.get_class(class_name).cloned();

                        if let Some(class_info) = class_info_opt {
                            // Check that all provided fields exist and have correct types
                            for field in fields {
                                if let Some(field_info) = class_info.fields.get(&field.name) {
                                    // Propagate expected type to field value for nested type deduction
                                    let expected_field_type = field_info.type_name.clone();
                                    let field_value_type = self.infer_expr_with_expected_type(&field.value, &Some(expected_field_type.clone()));

                                    if !field_value_type.is_assignable_to(&expected_field_type) {
                                        let (line, column) = Self::get_expr_span(&field.value);
                                        self.context.add_error_with_location(
                                            format!(
                                                "Field '{}' has wrong type: expected {}, got {}",
                                                field.name,
                                                expected_field_type.to_str(),
                                                field_value_type.to_str()
                                            ),
                                            line,
                                            column,
                                            None,
                                            None,
                                        );
                                    }
                                } else {
                                    self.context.add_error_with_location(
                                        format!("Unknown field '{}' for class '{}'", field.name, class_name),
                                        span.line,
                                        span.column,
                                        None,
                                        None,
                                    );
                                }
                            }
                            // Store the inferred type for code generation
                            self.context.expr_types.insert((span.line, span.column), class_name.clone());
                            // Return the class type
                            return Type::Class(class_name.clone());
                        }
                    }
                }
                // No expected type or not a class - infer from fields
                for field in fields {
                    self.infer_expr(&field.value);
                }
                Type::Unknown
            }
            Expr::Lambda { params, return_type: _, body, span } => {
                // Type check lambda with expected function type
                if let Some(expected) = expected_type {
                    if let Type::Function(expected_params, expected_return) = expected {
                        // Verify parameter count matches
                        if params.len() != expected_params.len() {
                            self.context.add_error_with_location(
                                format!(
                                    "Lambda expects {} parameters, but {} were provided",
                                    expected_params.len(),
                                    params.len()
                                ),
                                span.line,
                                span.column,
                                None,
                                None,
                            );
                        }

                        // Type check with expected types
                        let param_types: Vec<Type> = params.iter()
                            .zip(expected_params.iter())
                            .map(|(_p, expected_t)| {
                                expected_t.clone()
                            })
                            .collect();

                        let ret_type = expected_return.as_ref().clone();

                        // Add parameters as local variables with expected types
                        let mut added_vars = Vec::new();
                        for (param, param_type) in params.iter().zip(param_types.iter()) {
                            self.context.add_variable(&param.name, param_type.clone(), false);
                            added_vars.push(param.name.clone());
                        }

                        // Store expected return type
                        let old_return = self.context.current_method_return.clone();
                        self.context.current_method_return = Some(ret_type.clone());

                        // Check body
                        for stmt in body {
                            self.check_stmt(stmt);
                        }

                        // Clean up
                        for var in added_vars {
                            self.context.variables.remove(&var);
                        }
                        self.context.current_method_return = old_return;

                        return Type::Function(param_types, Box::new(ret_type.clone()));
                    }
                }
                // Fall back to regular inference
                self.infer_expr(expr)
            }
            _ => self.infer_expr(expr),
        }
    }

    fn check_function_call(&mut self, func_sig: &FunctionSignature, args: &[Expr], func_name: &str, span_line: usize, span_column: usize) {
        // Check argument count - allow fewer args if remaining params have defaults
        let min_args = func_sig.params.iter().position(|p| p.default).unwrap_or(func_sig.params.len());
        if args.len() > func_sig.params.len() {
            self.context.add_error_with_location(
                format!(
                    "Function '{}' expects {} arguments, got {}",
                    func_name,
                    func_sig.params.len(),
                    args.len()
                ),
                span_line,
                span_column,
                None,
                None,
            );
            return;
        }
        if args.len() < min_args {
            self.context.add_error_with_location(
                format!(
                    "Function '{}' requires at least {} arguments, got {}",
                    func_name,
                    min_args,
                    args.len()
                ),
                span_line,
                span_column,
                None,
                None,
            );
            return;
        }

        // Check argument types
        for (i, (arg, param)) in args.iter().zip(func_sig.params.iter()).enumerate() {
            let arg_type = self.infer_expr_with_expected_type(arg, &param.type_name);

            if let Some(expected_type) = &param.type_name {
                if !arg_type.is_assignable_to(expected_type) && arg_type != Type::Unknown {
                    let (line, column) = Self::get_expr_span(arg);
                    self.context.add_error_with_location(
                        format!(
                            "Argument {} of function '{}' has wrong type: expected {}, got {}",
                            i + 1,
                            func_name,
                            expected_type.to_str(),
                            arg_type.to_str()
                        ),
                        line,
                        column,
                        None,
                        None,
                    );
                }
            }
        }
    }

    fn check_method_call(&mut self, method_sig: &MethodSignature, args: &[Expr], method_name: &str, class_name: &str, span_line: usize, span_column: usize) {
        // Check argument count - allow fewer args if remaining params have defaults
        let min_args = method_sig.params.iter().position(|p| p.default).unwrap_or(method_sig.params.len());
        if args.len() > method_sig.params.len() {
            self.context.add_error_with_location(
                format!(
                    "Method '{}' on class '{}' expects {} arguments, got {}",
                    method_name,
                    class_name,
                    method_sig.params.len(),
                    args.len()
                ),
                span_line,
                span_column,
                None,
                None,
            );
            return;
        }
        if args.len() < min_args {
            self.context.add_error_with_location(
                format!(
                    "Method '{}' on class '{}' requires at least {} arguments, got {}",
                    method_name,
                    class_name,
                    min_args,
                    args.len()
                ),
                span_line,
                span_column,
                None,
                None,
            );
            return;
        }

        // Check argument types
        for (i, (arg, param)) in args.iter().zip(method_sig.params.iter()).enumerate() {
            let arg_type = self.infer_expr_with_expected_type(arg, &param.type_name);

            if let Some(expected_type) = &param.type_name {
                if !arg_type.is_assignable_to(expected_type) && arg_type != Type::Unknown {
                    let (line, column) = Self::get_expr_span(arg);
                    self.context.add_error_with_location(
                        format!(
                            "Argument {} of method '{}' has wrong type: expected {}, got {}",
                            i + 1,
                            method_name,
                            expected_type.to_str(),
                            arg_type.to_str()
                        ),
                        line,
                        column,
                        None,
                        None,
                    );
                }
            }
        }
    }
}
