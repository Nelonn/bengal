use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::c_char;
use sparkler::{VM, Value, PromiseState};
use std::sync::Arc;
use tokio::sync::Mutex;
use simd_json;

pub fn register_all(vm: &mut VM) {
    vm.register_native("print", native_print);
    vm.register_native("println", native_println);
    vm.register_native("http_get", native_http_get);
    vm.register_native("http_post", native_http_post);
    vm.register_native("std::io::print", native_print);
    vm.register_native("std::io::println", native_println);

    // JSON
    vm.register_native("std::json::stringify", native_json_stringify);
    vm.register_native("std::json::parse", native_json_parse);

    // Reflection
    vm.register_native("std::reflect::type_of", native_reflect_typeof);
    vm.register_native("std::reflect::class_name", native_reflect_class_name);
    vm.register_native("std::reflect::fields", native_reflect_fields);

    // Fallback function that throws an error

    vm.register_fallback(|_args| {
        Err("Native method not available or disabled by runtime".to_string())
    });
}

fn native_print(args: &mut Vec<Value>) -> Result<Value, String> {
    for arg in args {
        print!("{}", arg.to_string());
    }
    Ok(Value::Null)
}

fn native_println(args: &mut Vec<Value>) -> Result<Value, String> {
    for arg in args {
        print!("{}", arg.to_string());
    }
    println!();
    Ok(Value::Null)
}

fn native_http_get(args: &mut Vec<Value>) -> Result<Value, String> {
    if args.is_empty() {
        return Err("http_get requires URL argument".to_string());
    }
    let url = args[0].to_string();
    
    let promise = Arc::new(Mutex::new(PromiseState::Pending));
    let p_clone = promise.clone();
    
    tokio::spawn(async move {
        match http_get_async(&url).await {
            Ok(response) => {
                let mut state = p_clone.lock().await;
                *state = PromiseState::Resolved(Value::String(response));
            }
            Err(e) => {
                let mut state = p_clone.lock().await;
                *state = PromiseState::Rejected(e);
            }
        }
    });
    
    Ok(Value::Promise(promise))
}

fn native_http_post(args: &mut Vec<Value>) -> Result<Value, String> {
    if args.len() < 2 {
        return Err("http_post requires URL and body arguments".to_string());
    }
    let url = args[0].to_string();
    let body = args[1].to_string();
    
    let promise = Arc::new(Mutex::new(PromiseState::Pending));
    let p_clone = promise.clone();
    
    tokio::spawn(async move {
        match http_post_async(&url, &body).await {
            Ok(response) => {
                let mut state = p_clone.lock().await;
                *state = PromiseState::Resolved(Value::String(response));
            }
            Err(e) => {
                let mut state = p_clone.lock().await;
                *state = PromiseState::Rejected(e);
            }
        }
    });
    
    Ok(Value::Promise(promise))
}

// Async HTTP functions
pub async fn http_get_async(url: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    
    match client.get(url).send().await {
        Ok(response) => {
            match response.text().await {
                Ok(text) => Ok(text),
                Err(e) => Err(format!("Failed to read response: {}", e)),
            }
        }
        Err(e) => Err(format!("Request failed: {}", e)),
    }
}

pub async fn http_post_async(url: &str, body: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    
    match client.post(url)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await 
    {
        Ok(response) => {
            match response.text().await {
                Ok(text) => Ok(text),
                Err(e) => Err(format!("Failed to read response: {}", e)),
            }
        }
        Err(e) => Err(format!("Request failed: {}", e)),
    }
}

// HTTP Client configuration
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    pub base_url: Option<String>,
    pub timeout: u64,
    pub max_redirects: u32,
    pub redirect_policy: RedirectPolicy,
    pub proxy: Option<ProxyConfig>,
    pub verify_ssl: bool,
    pub default_headers: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RedirectPolicy {
    Follow,
    Limited(u32),
    None,
}

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            base_url: None,
            timeout: 30000,
            max_redirects: 10,
            redirect_policy: RedirectPolicy::Follow,
            proxy: None,
            verify_ssl: true,
            default_headers: HashMap::new(),
        }
    }
}

// Build a reqwest client from config
pub fn build_client(config: &HttpClientConfig) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(config.timeout))
        .danger_accept_invalid_certs(!config.verify_ssl);
    
    // Apply redirect policy
    match config.redirect_policy {
        RedirectPolicy::Follow => {
            builder = builder.redirect(reqwest::redirect::Policy::limited(config.max_redirects as usize));
        }
        RedirectPolicy::Limited(n) => {
            builder = builder.redirect(reqwest::redirect::Policy::limited(n as usize));
        }
        RedirectPolicy::None => {
            builder = builder.redirect(reqwest::redirect::Policy::none());
        }
    }
    
    // Apply proxy if configured
    if let Some(proxy_config) = &config.proxy {
        let proxy_url = if let (Some(username), Some(password)) = (&proxy_config.username, &proxy_config.password) {
            format!("http://{}:{}@{}:{}", username, password, proxy_config.host, proxy_config.port)
        } else {
            format!("http://{}:{}", proxy_config.host, proxy_config.port)
        };
        
        let proxy = reqwest::Proxy::http(&proxy_url)
            .map_err(|e| format!("Failed to create proxy: {}", e))?;
        builder = builder.proxy(proxy);
    }
    
    builder.build()
        .map_err(|e| format!("Failed to build client: {}", e))
}

// Parse method string to reqwest method
pub fn parse_method(method: &str) -> reqwest::Method {
    match method.to_uppercase().as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "DELETE" => reqwest::Method::DELETE,
        "PATCH" => reqwest::Method::PATCH,
        "HEAD" => reqwest::Method::HEAD,
        "OPTIONS" => reqwest::Method::OPTIONS,
        _ => reqwest::Method::GET,
    }
}

// Parse headers from string (format: "Key: Value\nKey2: Value2\n")
pub fn parse_headers(headers_str: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    for line in headers_str.lines() {
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    headers
}

// HTTP Client request
pub async fn http_client_request_async(
    config: &HttpClientConfig,
    method: &str,
    url: &str,
    headers_str: &str,
    body: Option<&str>,
) -> Result<HttpResponse, String> {
    let client = build_client(config)?;
    
    let full_url = if let Some(base) = &config.base_url {
        if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else {
            format!("{}{}", base.trim_end_matches('/'), url)
        }
    } else {
        url.to_string()
    };
    
    let req_method = parse_method(method);
    let mut req_builder = client.request(req_method, &full_url);
    
    // Add default headers from config
    for (key, value) in &config.default_headers {
        req_builder = req_builder.header(key, value);
    }
    
    // Add request-specific headers
    let request_headers = parse_headers(headers_str);
    for (key, value) in request_headers {
        req_builder = req_builder.header(&key, &value);
    }
    
    // Add body if present
    if let Some(body_content) = body {
        req_builder = req_builder.body(body_content.to_string());
    }
    
    let response = req_builder.send().await
        .map_err(|e| format!("Request failed: {}", e))?;
    
    let status = response.status().as_u16();
    let status_text = response.status().canonical_reason().unwrap_or("Unknown").to_string();
    let final_url = response.url().to_string();
    
    // Collect headers
    let mut response_headers = String::new();
    for (name, value) in response.headers() {
        response_headers.push_str(&format!("{}: {}\n", name, value.to_str().unwrap_or("")));
    }
    
    let response_body = response.text().await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    Ok(HttpResponse {
        status,
        status_text,
        headers: response_headers,
        body: response_body,
        url: final_url,
    })
}

#[derive(Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: String,
    pub body: String,
    pub url: String,
}

// JSON
fn native_json_stringify(args: &mut Vec<Value>) -> Result<Value, String> {
    if args.is_empty() {
        return Err("stringify requires at least one argument".to_string());
    }
    
    match simd_json::to_string(&args[0]) {
        Ok(s) => Ok(Value::String(s)),
        Err(e) => Err(format!("Failed to serialize: {}", e)),
    }
}

fn native_json_parse(args: &mut Vec<Value>) -> Result<Value, String> {
    if args.is_empty() {
        return Err("parse requires at least one argument".to_string());
    }
    
    let json_str = match &args[0] {
        Value::String(s) => s.clone(),
        _ => return Err("parse requires a string argument".to_string()),
    };
    
    let mut bytes = json_str.into_bytes();
    match simd_json::from_slice(&mut bytes) {
        Ok(v) => Ok(v),
        Err(e) => Err(format!("Failed to parse JSON: {}", e)),
    }
}

// Reflection
fn native_reflect_typeof(args: &mut Vec<Value>) -> Result<Value, String> {
    if args.is_empty() {
        return Err("typeof requires at least one argument".to_string());
    }
    
    let type_name = match &args[0] {
        Value::String(_) => "string",
        Value::Int8(_) | Value::Int16(_) | Value::Int32(_) | Value::Int64(_) |
        Value::UInt8(_) | Value::UInt16(_) | Value::UInt32(_) | Value::UInt64(_) => "int",
        Value::Float32(_) | Value::Float64(_) => "float",
        Value::Bool(_) => "bool",
        Value::Null => "null",
        Value::Instance(_) => "object",
        Value::Promise(_) => "promise",
    };
    
    Ok(Value::String(type_name.to_string()))
}

fn native_reflect_class_name(args: &mut Vec<Value>) -> Result<Value, String> {
    if args.is_empty() {
        return Err("class_name requires at least one argument".to_string());
    }
    
    match &args[0] {
        Value::Instance(inst) => Ok(Value::String(inst.class.clone())),
        _ => Ok(Value::Null),
    }
}

fn native_reflect_fields(args: &mut Vec<Value>) -> Result<Value, String> {
    if args.is_empty() {
        return Err("fields requires at least one argument".to_string());
    }
    
    match &args[0] {
        Value::Instance(inst) => {
            use sparkler::vm::Instance;
            Ok(Value::Instance(Instance {
                class: "Object".to_string(),
                fields: inst.fields.clone(),
            }))
        }
        _ => Ok(Value::Null),
    }
}

