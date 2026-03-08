use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::c_char;

pub const NATIVE_BENGAL_PRINT: u8 = 0;
pub const NATIVE_BENGAL_PRINTLN: u8 = 1;
pub const NATIVE_HTTP_GET: u8 = 2;
pub const NATIVE_HTTP_POST: u8 = 3;
pub const NATIVE_HTTP_CLIENT_REQUEST: u8 = 4;
pub const NATIVE_HTTP_CLIENT_GET: u8 = 5;
pub const NATIVE_HTTP_CLIENT_POST: u8 = 6;
pub const NATIVE_HTTP_CLIENT_GET_WITH_HEADERS: u8 = 7;
pub const NATIVE_HTTP_CLIENT_POST_WITH_HEADERS: u8 = 8;

pub fn get_native(name: &str) -> Option<NativeFn> {
    match name {
        "print" => Some(native_print),
        _ => None,
    }
}

pub type NativeFn = fn(&mut Vec<String>) -> Result<(), String>;

fn native_print(args: &mut Vec<String>) -> Result<(), String> {
    if args.is_empty() {
        return Err("print() requires at least 1 argument".to_string());
    }

    let s = args.remove(0);
    print!("{}", s);
    Ok(())
}

pub fn call_native_by_id(id: u8, args: &mut Vec<String>) -> Result<(), String> {
    match id {
        NATIVE_BENGAL_PRINT => native_bengal_print(args),
        NATIVE_BENGAL_PRINTLN => native_bengal_println(args),
        _ => Err(format!("Unknown native function ID: {}", id)),
    }
}

fn native_bengal_print(args: &mut Vec<String>) -> Result<(), String> {
    if args.is_empty() {
        return Err("bengal_print() requires at least 1 argument".to_string());
    }
    let s = args.remove(0);
    print!("{}", s);
    Ok(())
}

fn native_bengal_println(args: &mut Vec<String>) -> Result<(), String> {
    if args.is_empty() {
        return Err("bengal_println() requires at least 1 argument".to_string());
    }
    let s = args.remove(0);
    println!("{}", s);
    Ok(())
}

#[no_mangle]
pub extern "C" fn bengal_print(s: *const c_char) {
    unsafe {
        if let Ok(c_str) = CStr::from_ptr(s).to_str() {
            print!("{}", c_str);
        }
    }
}

#[no_mangle]
pub extern "C" fn bengal_println(s: *const c_char) {
    unsafe {
        if let Ok(c_str) = CStr::from_ptr(s).to_str() {
            println!("{}", c_str);
        }
    }
}

#[no_mangle]
pub extern "C" fn bengal_init() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn bengal_exit(code: i32) {
    std::process::exit(code);
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

// Helper function to get async native function result
pub async fn call_native_async_by_id(id: u8, args: &[String]) -> Result<String, String> {
    match id {
        NATIVE_HTTP_GET => {
            let url = args.first().ok_or("http_get requires URL argument")?;
            http_get_async(url).await
        }
        NATIVE_HTTP_POST => {
            let url = args.first().ok_or("http_post requires URL argument")?;
            let body = args.get(1).map(|s| s.as_str()).unwrap_or("");
            http_post_async(url, body).await
        }
        NATIVE_HTTP_CLIENT_GET => {
            let url = args.first().ok_or("http_client_get requires URL argument")?;
            let config = HttpClientConfig::default();
            match http_client_request_async(&config, "GET", url, "", None).await {
                Ok(response) => Ok(response.body),
                Err(e) => Err(e),
            }
        }
        NATIVE_HTTP_CLIENT_POST => {
            let url = args.first().ok_or("http_client_post requires URL argument")?;
            let body = args.get(1).map(|s| s.as_str()).unwrap_or("");
            let config = HttpClientConfig::default();
            match http_client_request_async(&config, "POST", url, "", Some(body)).await {
                Ok(response) => Ok(response.body),
                Err(e) => Err(e),
            }
        }
        NATIVE_HTTP_CLIENT_GET_WITH_HEADERS => {
            let url = args.first().ok_or("http_client_get_with_headers requires URL argument")?;
            let headers = args.get(1).map(|s| s.as_str()).unwrap_or("");
            let config = HttpClientConfig::default();
            match http_client_request_async(&config, "GET", url, headers, None).await {
                Ok(response) => Ok(response.body),
                Err(e) => Err(e),
            }
        }
        NATIVE_HTTP_CLIENT_POST_WITH_HEADERS => {
            let url = args.first().ok_or("http_client_post_with_headers requires URL argument")?;
            let headers = args.get(1).map(|s| s.as_str()).unwrap_or("");
            let body = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let config = HttpClientConfig::default();
            match http_client_request_async(&config, "POST", url, headers, Some(body)).await {
                Ok(response) => Ok(response.body),
                Err(e) => Err(e),
            }
        }
        _ => Err(format!("Unknown async native function ID: {}", id)),
    }
}

// FFI Type Conversion Functions
// These functions allow converting Bengal's i64/f64 to specific C types

/// Convert i64 to u8 (wraps on overflow)
pub fn ffi_to_u8(value: i64) -> u8 {
    value as u8
}

/// Convert i64 to i8 (wraps on overflow)
pub fn ffi_to_i8(value: i64) -> i8 {
    value as i8
}

/// Convert i64 to u16 (wraps on overflow)
pub fn ffi_to_u16(value: i64) -> u16 {
    value as u16
}

/// Convert i64 to i16 (wraps on overflow)
pub fn ffi_to_i16(value: i64) -> i16 {
    value as i16
}

/// Convert i64 to u32 (wraps on overflow)
pub fn ffi_to_u32(value: i64) -> u32 {
    value as u32
}

/// Convert i64 to i32 (wraps on overflow)
pub fn ffi_to_i32(value: i64) -> i32 {
    value as i32
}

/// Convert i64 to u64 (wraps on overflow)
pub fn ffi_to_u64(value: i64) -> u64 {
    value as u64
}

/// Convert f64 to f32 (may lose precision)
pub fn ffi_to_f32(value: f64) -> f32 {
    value as f32
}

/// Convert u8 to i64
pub fn ffi_from_u8(value: u8) -> i64 {
    value as i64
}

/// Convert i8 to i64
pub fn ffi_from_i8(value: i8) -> i64 {
    value as i64
}

/// Convert u16 to i64
pub fn ffi_from_u16(value: u16) -> i64 {
    value as i64
}

/// Convert i16 to i64
pub fn ffi_from_i16(value: i16) -> i64 {
    value as i64
}

/// Convert u32 to i64
pub fn ffi_from_u32(value: u32) -> i64 {
    value as i64
}

/// Convert i32 to i64
pub fn ffi_from_i32(value: i32) -> i64 {
    value as i64
}

/// Convert u64 to i64 (wraps on overflow)
pub fn ffi_from_u64(value: u64) -> i64 {
    value as i64
}

/// Convert f32 to f64
pub fn ffi_from_f32(value: f32) -> f64 {
    value as f64
}

// C FFI exports for external use
#[no_mangle]
pub extern "C" fn bengal_ffi_to_u8(value: i64) -> u8 {
    ffi_to_u8(value)
}

#[no_mangle]
pub extern "C" fn bengal_ffi_to_i8(value: i64) -> i8 {
    ffi_to_i8(value)
}

#[no_mangle]
pub extern "C" fn bengal_ffi_to_u16(value: i64) -> u16 {
    ffi_to_u16(value)
}

#[no_mangle]
pub extern "C" fn bengal_ffi_to_i16(value: i64) -> i16 {
    ffi_to_i16(value)
}

#[no_mangle]
pub extern "C" fn bengal_ffi_to_u32(value: i64) -> u32 {
    ffi_to_u32(value)
}

#[no_mangle]
pub extern "C" fn bengal_ffi_to_i32(value: i64) -> i32 {
    ffi_to_i32(value)
}

#[no_mangle]
pub extern "C" fn bengal_ffi_to_u64(value: i64) -> u64 {
    ffi_to_u64(value)
}

#[no_mangle]
pub extern "C" fn bengal_ffi_to_f32(value: f64) -> f32 {
    ffi_to_f32(value)
}

#[no_mangle]
pub extern "C" fn bengal_ffi_from_u8(value: u8) -> i64 {
    ffi_from_u8(value)
}

#[no_mangle]
pub extern "C" fn bengal_ffi_from_i8(value: i8) -> i64 {
    ffi_from_i8(value)
}

#[no_mangle]
pub extern "C" fn bengal_ffi_from_u16(value: u16) -> i64 {
    ffi_from_u16(value)
}

#[no_mangle]
pub extern "C" fn bengal_ffi_from_i16(value: i16) -> i64 {
    ffi_from_i16(value)
}

#[no_mangle]
pub extern "C" fn bengal_ffi_from_u32(value: u32) -> i64 {
    ffi_from_u32(value)
}

#[no_mangle]
pub extern "C" fn bengal_ffi_from_i32(value: i32) -> i64 {
    ffi_from_i32(value)
}

#[no_mangle]
pub extern "C" fn bengal_ffi_from_u64(value: u64) -> i64 {
    ffi_from_u64(value)
}

#[no_mangle]
pub extern "C" fn bengal_ffi_from_f32(value: f32) -> f64 {
    ffi_from_f32(value)
}
