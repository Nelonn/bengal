use std::collections::HashMap;
use crate::parser::{ClassDef, Method, Expr, Literal, Stmt, CastType};

fn is_numeric_type(ty: &Type) -> bool {
    ty.is_numeric()
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
    Promise(Box<Type>),
    Array(Box<Type>),
    Null,
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

        // Handle function types: (param_types) -> return_type or async (params) -> return_type
        let s_trimmed = s.trim_start_matches("async ");
        let is_async = s.starts_with("async ");
        
        if s_trimmed.starts_with('(') {
            if let Some(arrow_pos) = s_trimmed.find(") -> ") {
                let params_str = &s_trimmed[1..arrow_pos];
                let return_str = &s_trimmed[arrow_pos + 4..];

                let param_types: Vec<Type> = if params_str.trim().is_empty() {
                    Vec::new()
                } else {
                    params_str.split(',')
                        .map(|p| Type::from_str(p.trim()))
                        .collect()
                };

                let inner_type = Type::from_str(return_str.trim());
                let return_type = if is_async {
                    Box::new(Type::Promise(Box::new(inner_type)))
                } else {
                    Box::new(inner_type)
                };
                return Type::Function(param_types, return_type);
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
            "self" => Type::SelfType,
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
            Type::Promise(t) => format!("Promise<{}>", t.to_str()),
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
                format!("({}) -> {}", params_str.join(", "), return_type.to_str())
            }
            Type::SelfType => "self".to_string(),
        }
    }

    pub fn is_assignable_to(&self, other: &Type) -> bool {
        match (self, other) {
            (Type::Null, Type::Optional(_)) => true,
            (Type::Null, Type::Promise(_)) => true,
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
    pub is_async: bool,
    pub is_native: bool,
    /// Mangled name for VM lookup (includes type information for overloading)
    pub mangled_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParamSignature {
    pub name: String,
    pub type_name: Option<Type>,
}

/// Generate a mangled name for a function based on its parameter types
/// This enables function overloading by creating unique names for each signature
/// Format: <name>(<type1>,<type2>,...) - simple and readable mangling
pub fn mangle_function_name(name: &str, param_types: &[Type]) -> String {
    let mut mangled = String::new();
    mangled.push_str(name);
    mangled.push('(');
    
    for (i, ty) in param_types.iter().enumerate() {
        if i > 0 {
            mangled.push(',');
        }
        mangled.push_str(&ty.to_str());
    }
    
    mangled.push(')');
    mangled
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
    pub is_async: bool,
    pub is_native: bool,
    pub is_static: bool,
    pub mangled_name: Option<String>,
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
pub struct TypeAliasInfo {
    pub name: String,
    pub type_params: Vec<String>,
    pub aliased_type: Type,
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
    pub current_method_return: Option<Type>,
    pub current_async_inner_return: Option<Type>,
    pub current_method_params: Vec<String>,
    pub imports: Vec<String>,
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
            current_method_return: None,
            current_async_inner_return: None,
            current_method_params: Vec::new(),
            imports: Vec::new(),
            errors: Vec::new(),
            expr_types: HashMap::new(),
        };

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
            }],
            return_type: Some(Type::Str),
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
            mangled_name: None,
        };
        self.add_function("std.json.stringify", json_stringify);

        let json_parse = FunctionSignature {
            name: "std.json.parse".to_string(),
            params: vec![ParamSignature {
                name: "json".to_string(),
                type_name: Some(Type::Str),
            }],
            return_type: Some(Type::Unknown),
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
            mangled_name: None,
        };
        self.add_function("std.json.parse", json_parse);

        self.imports.push("std.json".to_string());

        // Reflection
        let reflect_typeof = FunctionSignature {
            name: "std.reflect.type_of".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
            }],
            return_type: Some(Type::Str),
            return_optional: false,
            is_method: false,
            is_async: false,
            is_native: true,
            mangled_name: None,
        };
        self.add_function("std.reflect.type_of", reflect_typeof);

        let reflect_class_name = FunctionSignature {
            name: "std.reflect.class_name".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
            }],
            return_type: Some(Type::Str),
            return_optional: true,
            is_method: false,
            is_async: false,
            is_native: true,
            mangled_name: None,
        };
        self.add_function("std.reflect.class_name", reflect_class_name);

        let reflect_fields = FunctionSignature {
            name: "std.reflect.fields".to_string(),
            params: vec![ParamSignature {
                name: "value".to_string(),
                type_name: Some(Type::Unknown),
            }],
            return_type: Some(Type::Unknown),
            return_optional: true,
            is_method: false,
            is_async: false,
            is_native: true,
            mangled_name: None,
        };
        self.add_function("std.reflect.fields", reflect_fields);

        self.imports.push("std.reflect".to_string());

        // Built-in types methods
        // str methods
        let mut str_methods = HashMap::new();
        str_methods.insert("length".to_string(), MethodSignature {
            name: "length".to_string(),
            params: vec![],
            return_type: Some(Type::Int),
            return_optional: false,
            private: false,
            is_async: false,
            is_native: true,
            is_static: false,
            mangled_name: None,
        });
        str_methods.insert("trim".to_string(), MethodSignature {
            name: "trim".to_string(),
            params: vec![],
            return_type: Some(Type::Str),
            return_optional: false,
            private: false,
            is_async: false,
            is_native: true,
            is_static: false,
            mangled_name: None,
        });
        str_methods.insert("split".to_string(), MethodSignature {
            name: "split".to_string(),
            params: vec![ParamSignature { name: "delimiter".to_string(), type_name: Some(Type::Str) }],
            return_type: Some(Type::Array(Box::new(Type::Str))),
            return_optional: false,
            private: false,
            is_async: false,
            is_native: true,
            is_static: false,
            mangled_name: None,
        });
        let mut str_method_overloads = HashMap::new();
        str_method_overloads.insert("length".to_string(), vec!["length()".to_string()]);
        str_method_overloads.insert("trim".to_string(), vec!["trim()".to_string()]);
        str_method_overloads.insert("split".to_string(), vec!["split(str)".to_string()]);
        self.classes.insert("str".to_string(), ClassInfo {
            name: "str".to_string(),
            fields: HashMap::new(),
            methods: str_methods,
            method_overloads: str_method_overloads,
            vtable: vec!["length".to_string(), "trim".to_string(), "split".to_string()],
            is_native: true,
            is_interface: false,
            parent_interfaces: vec![],
            type_params: vec![],
        });

        // Array methods
        let mut array_methods = HashMap::new();
        array_methods.insert("length".to_string(), MethodSignature {
            name: "length".to_string(),
            params: vec![],
            return_type: Some(Type::Int),
            return_optional: false,
            private: false,
            is_async: false,
            is_native: true,
            is_static: false,
            mangled_name: None,
        });
        array_methods.insert("add".to_string(), MethodSignature {
            name: "add".to_string(),
            params: vec![ParamSignature { name: "element".to_string(), type_name: Some(Type::Unknown) }],
            return_type: None,
            return_optional: false,
            private: false,
            is_async: false,
            is_native: true,
            is_static: false,
            mangled_name: None,
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
            parent_interfaces: vec![],
            type_params: vec!["T".to_string()],
        });
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
                is_async: method.is_async,
                is_native: method.is_native,
                is_static: method.is_static,
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
                is_async: false,
                is_native: false,
                is_static: false,
                mangled_name: Some(empty_ctor_mangled),
            });

            // Constructor with all fields as parameters
            if !class.fields.is_empty() {
                let field_params: Vec<ParamSignature> = class.fields.iter().map(|field| ParamSignature {
                    name: field.name.clone(),
                    type_name: Some(Type::from_str(&field.type_name)),
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
                    is_async: false,
                    is_native: false,
                    is_static: false,
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
        });
    }

    pub fn add_interface(&mut self, interface: &crate::parser::InterfaceDef) {
        let mut methods = HashMap::new();
        let mut method_overloads: HashMap<String, Vec<String>> = HashMap::new();
        for method in &interface.methods {
            let params: Vec<ParamSignature> = method.params.iter().map(|p| ParamSignature {
                name: p.name.clone(),
                type_name: p.type_name.as_ref().map(|t| Type::from_str(t)),
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
                is_async: method.is_async,
                is_native: method.is_native,
                is_static: method.is_static,
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
                return self.functions.get(first_mangled);
            }
        }

        // Try to find a function that ends with ::<name> or .<name>
        // Prefer exact module match if we're in a module context
        for (func_name, sig) in &self.functions {
            if func_name.ends_with(&format!("::{}", name)) || func_name.ends_with(&format!(".{}", name)) {
                return Some(sig);
            }
        }

        None
    }

    /// Try to resolve a module-qualified function name (e.g., "math.sin" with import "std.math")
    pub fn resolve_qualified_function(&self, module_alias: &str, member_name: &str) -> Option<&FunctionSignature> {
        // First try direct lookup
        let direct_name = format!("{}.{}", module_alias, member_name);
        if let Some(mangled_names) = self.function_overloads.get(&direct_name) {
            if let Some(first_mangled) = mangled_names.first() {
                return self.functions.get(first_mangled);
            }
        }

        // Try to find an import that matches the module alias
        for import_path in &self.imports {
            // Case 1: Import ends with the alias (e.g., import std.math and access math.sin)
            if let Some(last_dot) = import_path.rfind('.') {
                let import_alias = &import_path[last_dot + 1..];
                if import_alias == module_alias {
                    let qualified_name = format!("{}.{}", import_path, member_name);
                    if let Some(mangled_names) = self.function_overloads.get(&qualified_name) {
                        if let Some(first_mangled) = mangled_names.first() {
                            return self.functions.get(first_mangled);
                        }
                    }
                }
            } else if import_path == module_alias {
                // Import is exactly the alias (no dots, e.g., import math and access math.sin)
                let qualified_name = format!("{}.{}", import_path, member_name);
                if let Some(mangled_names) = self.function_overloads.get(&qualified_name) {
                    if let Some(first_mangled) = mangled_names.first() {
                        return self.functions.get(first_mangled);
                    }
                }
            }

            // Case 2: Import is a parent module (e.g., import std and access io.println -> std.io.println)
            let qualified_name = format!("{}.{}.{}", import_path, module_alias, member_name);
            if let Some(mangled_names) = self.function_overloads.get(&qualified_name) {
                if let Some(first_mangled) = mangled_names.first() {
                    return self.functions.get(first_mangled);
                }
            }
        }

        None
    }

    /// Try to resolve a module-qualified class name (e.g., "http.HttpClient" with import "std.http")
    pub fn resolve_qualified_class(&self, module_alias: &str, class_name: &str) -> Option<String> {
        // First try direct lookup
        let direct_name = format!("{}.{}", module_alias, class_name);
        if self.classes.contains_key(&direct_name) {
            return Some(direct_name);
        }

        // Try to find an import that matches the module alias
        for import_path in &self.imports {
            // Case 1: Import ends with the alias
            if let Some(last_dot) = import_path.rfind('.') {
                let import_alias = &import_path[last_dot + 1..];
                if import_alias == module_alias {
                    let qualified_name = format!("{}.{}", import_path, class_name);
                    if self.classes.contains_key(&qualified_name) {
                        return Some(qualified_name);
                    }
                }
            } else if import_path == module_alias {
                let qualified_name = format!("{}.{}", import_path, class_name);
                if self.classes.contains_key(&qualified_name) {
                    return Some(qualified_name);
                }
            }

            // Case 2: Import is a parent module
            let qualified_name = format!("{}.{}.{}", import_path, module_alias, class_name);
            if self.classes.contains_key(&qualified_name) {
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
            return Some(var);
        }

        // Try to find an import that matches the module alias
        for import_path in &self.imports {
            // Case 1: Import ends with the alias
            if let Some(last_dot) = import_path.rfind('.') {
                let import_alias = &import_path[last_dot + 1..];
                if import_alias == module_alias {
                    let qualified_name = format!("{}.{}", import_path, var_name);
                    if let Some(var) = self.variables.get(&qualified_name) {
                        return Some(var);
                    }
                }
            } else if import_path == module_alias {
                let qualified_name = format!("{}.{}", import_path, var_name);
                if let Some(var) = self.variables.get(&qualified_name) {
                    return Some(var);
                }
            }

            // Case 2: Import is a parent module
            let qualified_name = format!("{}.{}.{}", import_path, module_alias, var_name);
            if let Some(var) = self.variables.get(&qualified_name) {
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
                return Some(res);
            }
        }

        // 2. Try with imports
        for import_path in &self.imports {
            let qualified = format!("{}.{}", import_path, name);
            if let Some(overloads) = self.function_overloads.get(&qualified) {
                if let Some(res) = find_best_match(overloads) {
                    return Some(res);
                }
            }

            // Also try parent module sub-access (e.g., import std and access io.println -> std.io.println)
            if let Some(dot_pos) = name.find('.') {
                let (prefix, rest) = name.split_at(dot_pos);
                let qualified2 = format!("{}.{}.{}", import_path, prefix, &rest[1..]);
                if let Some(overloads) = self.function_overloads.get(&qualified2) {
                    if let Some(res) = find_best_match(overloads) {
                        return Some(res);
                    }
                }
            }
        }

        // 3. Try qualified lookup (fallback for older code)
        for (func_mangled, sig) in &self.functions {
            // Extract base name from mangled name (part before '(')
            let base_name = match func_mangled.find('(') {
                Some(pos) => &func_mangled[..pos],
                None => func_mangled,
            };

            if base_name.ends_with(&format!(".{}", name)) || base_name.ends_with(&format!("::{}", name)) {
                if self.signature_matches(sig, arg_types) {
                    return Some(sig);
                }
            }
        }

        None
    }

    /// Check if a function signature matches the given argument types
    fn signature_matches(&self, sig: &FunctionSignature, arg_types: &[Type]) -> bool {
        if sig.params.len() != arg_types.len() {
            return false;
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
                Stmt::Interface(interface) => {
                    self.context.add_interface(interface);
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
                        }).collect();

                        self.context.add_function(&func.name, FunctionSignature {
                            name: func.name.clone(),
                            params,
                            return_type: func.return_type.as_ref().map(|t| Type::from_str(t)),
                            return_optional: func.return_optional,
                            is_method: false,
                            is_async: func.is_async,
                            is_native: func.is_native,
                            mangled_name: None,
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
            Stmt::Let { name, type_annotation, expr } => {
                // If there's a type annotation, use it for type deduction
                let expr_type = if let Some(ref type_name) = type_annotation {
                    let expected_type = Type::from_str(type_name);
                    self.infer_expr_with_expected_type(expr, &Some(expected_type))
                } else {
                    self.infer_expr(expr)
                };
                self.context.add_variable(name, expr_type);
            }
            Stmt::Assign { name, expr, span } => {
                let expr_type = self.infer_expr(expr);

                if let Some(var_info) = self.context.get_variable(name) {
                    // Use expected type for type deduction
                    let expected_type = var_info.type_name.clone();
                    let deduced_type = self.infer_expr_with_expected_type(expr, &Some(expected_type.clone()));
                    
                    if !deduced_type.is_assignable_to(&expected_type) {
                        self.context.add_error(
                            format!(
                                "Type mismatch: cannot assign {} to variable '{}' of type {}",
                                deduced_type.to_str(),
                                name,
                                expected_type.to_str()
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
                        // Use expected type for type deduction
                        let expr_type = self.infer_expr_with_expected_type(e, &expected_return);
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
            Stmt::Continue => {
                // Continue statement - no type checking needed
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

    pub fn infer_expr(&mut self, expr: &Expr) -> Type {
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
                    for import_path in &self.context.imports {
                        if let Some(last_dot) = import_path.rfind('.') {
                            let import_alias = &import_path[last_dot + 1..];
                            if import_alias == name {
                                is_module_alias = true;
                                break;
                            }
                        } else if import_path == name {
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
                    crate::parser::BinaryOp::Multiply | crate::parser::BinaryOp::Divide |
                    crate::parser::BinaryOp::Modulo => {
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
                    crate::parser::BinaryOp::Pow => {
                        // Power operation requires numeric types and returns float
                        if !is_numeric_type(&left_type) && left_type != Type::Unknown {
                            self.context.add_error(
                                format!("Expected numeric type for power operation, got {}", left_type.to_str()),
                                0
                            );
                        }
                        if !is_numeric_type(&right_type) && right_type != Type::Unknown {
                            self.context.add_error(
                                format!("Expected numeric type for power operation, got {}", right_type.to_str()),
                                0
                            );
                        }
                        Type::Float
                    }
                    crate::parser::BinaryOp::Greater | crate::parser::BinaryOp::Less |
                    crate::parser::BinaryOp::GreaterEqual | crate::parser::BinaryOp::LessEqual => {
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
                    crate::parser::UnaryOp::PrefixIncrement | crate::parser::UnaryOp::PostfixIncrement => {
                        // Increment operator works on numeric types and returns the original type
                        if inner_type != Type::Int && inner_type != Type::Float && inner_type != Type::Unknown {
                            self.context.add_error(
                                format!("Expected numeric type for ++ operator, got {}", inner_type.to_str()),
                                0
                            );
                        }
                        inner_type
                    }
                    crate::parser::UnaryOp::PrefixDecrement | crate::parser::UnaryOp::PostfixDecrement | crate::parser::UnaryOp::Decrement => {
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
            Expr::Call { callee, args, span } => {
                if let Expr::Variable { name: func_name, .. } = callee.as_ref() {
                    // Check if it's a function call - use overload resolution
                    // First, infer argument types for overload resolution
                    let arg_types: Vec<Type> = args.iter()
                        .map(|arg| self.infer_expr(arg))
                        .collect();
                    
                    // Use resolve_function_call for proper overload resolution
                    let func_sig = self.context.resolve_function_call(func_name, &arg_types);
                    if let Some(sig) = func_sig.cloned() {
                        self.check_function_call(&sig, args, func_name);
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
                    } else if let Some(_class_info) = self.context.get_class(func_name) {
                        // It's a class instantiation - check constructor args
                        let ctor_sig = self.context.resolve_method_call(func_name, "constructor", &arg_types);
                        if let Some(sig) = ctor_sig.cloned() {
                            self.check_method_call(&sig, args, "constructor", func_name);
                        } else if !args.is_empty() {
                            // No explicit constructor but args were provided
                            self.context.add_error_with_location(
                                format!("Class '{}' does not accept constructor arguments", func_name),
                                span.line, span.column, None, None
                            );
                        }
                        // Return type is the class type
                        Type::Class(func_name.clone())
                    } else {
                        self.context.add_error_with_location(
                            format!("Undefined function: '{}'", func_name),
                            span.line, span.column, None, None
                        );
                        Type::Unknown
                    }
                } else if let Expr::Get { object, name, span: method_span } = callee.as_ref() {
                    // Could be method call OR module.function() call
                    let object_type = self.infer_expr(object);

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

                            self.check_method_call(&sig, args, name, &class_name);
                            let mut return_type = sig.return_type.clone().unwrap_or(Type::Unknown);
                            // If calling an async method, return type is Promise<T>
                            if sig.is_async {
                                if let Type::Promise(_) = return_type {
                                    // Already a Promise type
                                } else {
                                    return_type = Type::Promise(Box::new(return_type));
                                }
                            }
                            return return_type;
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
                            self.check_function_call(&sig, args, &sig.name);
                            let mut return_type = sig.return_type.clone().unwrap_or(Type::Unknown);
                            if sig.is_async {
                                if let Type::Promise(_) = return_type {
                                } else {
                                    return_type = Type::Promise(Box::new(return_type));
                                }
                            }
                            return_type
                        } else if let Some(qualified_class_name) = self.context.resolve_qualified_class(module_name, name) {
                            // It's a qualified class instantiation
                            let arg_types: Vec<Type> = args.iter()
                                .map(|arg| self.infer_expr(arg))
                                .collect();
                            let ctor_sig = self.context.resolve_method_call(&qualified_class_name, "constructor", &arg_types);
                            if let Some(sig) = ctor_sig.cloned() {
                                self.check_method_call(&sig, args, "constructor", &qualified_class_name);
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

                    for import_path in &self.context.imports {
                        let qualified_enum = format!("{}.{}.{}", import_path, module_name, name);
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
            Expr::Await { expr, span } => {
                let inner_type = self.infer_expr(expr);
                // Await unwraps Promise<T> to T
                match inner_type {
                    Type::Promise(t) => *t,
                    Type::Unknown => Type::Unknown,
                    _ => {
                        self.context.add_error_with_location(
                            format!("Can only await Promise values, got {}", inner_type.to_str()),
                            span.line, span.column, None, None
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
                    CastType::Int | CastType::Int8 | CastType::UInt8 | CastType::Int16 | CastType::UInt16 | 
                    CastType::Int32 | CastType::UInt32 | CastType::Int64 | CastType::UInt64 => {
                        // int() can accept: float, str, bool, int, and other numeric types
                        if !inner_type.is_numeric() && 
                           inner_type != Type::Str && 
                           inner_type != Type::Bool && 
                           inner_type != Type::Unknown {
                            self.context.add_error(
                                format!("Cannot cast {} to integer type", inner_type.to_str()),
                                0
                            );
                        }
                    }
                    CastType::Float | CastType::Float32 | CastType::Float64 => {
                        // float() can accept: int, str, bool, float, and other numeric types
                        if !inner_type.is_numeric() && 
                           inner_type != Type::Str && 
                           inner_type != Type::Bool &&
                           inner_type != Type::Unknown {
                            self.context.add_error(
                                format!("Cannot cast {} to float type", inner_type.to_str()),
                                0
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
            Expr::Index { object, index, .. } => {
                let object_type = self.infer_expr(object);
                let index_type = self.infer_expr(index);

                if index_type != Type::Int && index_type != Type::Unknown {
                    self.context.add_error(
                        format!("Array index must be an integer, got {}", index_type.to_str()),
                        0
                    );
                }

                match object_type {
                    Type::Array(inner) => *inner,
                    Type::Str => Type::Str,
                    Type::Unknown => Type::Unknown,
                    _ => {
                        self.context.add_error(
                            format!("Type {} does not support indexing", object_type.to_str()),
                            0
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
            Expr::Lambda { params, return_type, body, is_async, .. } => {
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
                    self.context.add_variable(&param.name, param_type);
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

                // For async lambdas, wrap return type in Promise
                let final_ret_type = if *is_async {
                    Type::Promise(Box::new(ret_type))
                } else {
                    ret_type
                };

                Type::Function(param_types, Box::new(final_ret_type))
            }
        }
    }

    /// Infer expression type with an expected type hint (for type deduction)
    fn infer_expr_with_expected_type(&mut self, expr: &Expr, expected_type: &Option<Type>) -> Type {
        match expr {
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
                                        self.context.add_error(
                                            format!(
                                                "Field '{}' has wrong type: expected {}, got {}",
                                                field.name,
                                                expected_field_type.to_str(),
                                                field_value_type.to_str()
                                            ),
                                            0
                                        );
                                    }
                                } else {
                                    self.context.add_error(
                                        format!("Unknown field '{}' for class '{}'", field.name, class_name),
                                        0
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
            Expr::Lambda { params, return_type: _, body, is_async, .. } => {
                // Type check lambda with expected function type
                if let Some(expected) = expected_type {
                    if let Type::Function(expected_params, expected_return) = expected {
                        // Verify parameter count matches
                        if params.len() != expected_params.len() {
                            self.context.add_error(
                                format!(
                                    "Lambda expects {} parameters, but {} were provided",
                                    expected_params.len(),
                                    params.len()
                                ),
                                0
                            );
                        }

                        // Type check with expected types
                        let param_types: Vec<Type> = params.iter()
                            .zip(expected_params.iter())
                            .map(|(_p, expected_t)| {
                                expected_t.clone()
                            })
                            .collect();

                        // For async lambdas, expected return should be Promise<T>
                        // Extract inner type from Promise if async
                        let ret_type = if *is_async {
                            if let Type::Promise(inner) = expected_return.as_ref() {
                                inner.as_ref().clone()
                            } else {
                                expected_return.as_ref().clone()
                            }
                        } else {
                            expected_return.as_ref().clone()
                        };

                        // Add parameters as local variables with expected types
                        let mut added_vars = Vec::new();
                        for (param, param_type) in params.iter().zip(param_types.iter()) {
                            self.context.add_variable(&param.name, param_type.clone());
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

                        // Return function type with Promise wrapper for async
                        let final_ret_type = if *is_async {
                            Type::Promise(Box::new(ret_type))
                        } else {
                            ret_type.clone()
                        };

                        return Type::Function(param_types, Box::new(final_ret_type));
                    }
                }
                // Fall back to regular inference
                self.infer_expr(expr)
            }
            _ => self.infer_expr(expr),
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
            let arg_type = self.infer_expr_with_expected_type(arg, &param.type_name);

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
            let arg_type = self.infer_expr_with_expected_type(arg, &param.type_name);

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
