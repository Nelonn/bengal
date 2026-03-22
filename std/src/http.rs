use sparkler::Value;
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub fn native_http_get(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String("http_get requires URL argument".to_string()));
    }
    let url = args[0].to_string();

    match http_get(&url) {
        Ok(response) => Ok(Value::String(response)),
        Err(e) => Err(Value::String(e)),
    }
}

pub fn native_http_post(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.len() < 2 {
        return Err(Value::String(
            "http_post requires URL and body arguments".to_string(),
        ));
    }
    let url = args[0].to_string();
    let body = args[1].to_string();

    match http_post(&url, &body) {
        Ok(response) => Ok(Value::String(response)),
        Err(e) => Err(Value::String(e)),
    }
}

// Synchronous HTTP functions using blocking reqwest
pub fn http_get(url: &str) -> Result<String, String> {
    let url = url.to_string();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(http_get_async(&url))
    }).join().unwrap()
}

pub fn http_post(url: &str, body: &str) -> Result<String, String> {
    let url = url.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(http_post_async(&url, &body))
    }).join().unwrap()
}

// Async HTTP functions (used internally with blocking)
pub async fn http_get_async(url: &str) -> Result<String, String> {
    let client = reqwest::Client::new();

    match client.get(url).send().await {
        Ok(response) => match response.text().await {
            Ok(text) => Ok(text),
            Err(e) => Err(format!("Failed to read response: {}", e)),
        },
        Err(e) => Err(format!("Request failed: {}", e)),
    }
}

pub async fn http_post_async(url: &str, body: &str) -> Result<String, String> {
    let client = reqwest::Client::new();

    match client
        .post(url)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
    {
        Ok(response) => match response.text().await {
            Ok(text) => Ok(text),
            Err(e) => Err(format!("Failed to read response: {}", e)),
        },
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

pub fn build_client(config: &HttpClientConfig) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(config.timeout))
        .danger_accept_invalid_certs(!config.verify_ssl);

    match config.redirect_policy {
        RedirectPolicy::Follow => {
            builder = builder.redirect(reqwest::redirect::Policy::limited(
                config.max_redirects as usize,
            ));
        }
        RedirectPolicy::Limited(n) => {
            builder = builder.redirect(reqwest::redirect::Policy::limited(n as usize));
        }
        RedirectPolicy::None => {
            builder = builder.redirect(reqwest::redirect::Policy::none());
        }
    }

    if let Some(proxy_config) = &config.proxy {
        let proxy_url = if let (Some(username), Some(password)) =
            (&proxy_config.username, &proxy_config.password)
        {
            format!(
                "http://{}:{}@{}:{}",
                username, password, proxy_config.host, proxy_config.port
            )
        } else {
            format!("http://{}:{}", proxy_config.host, proxy_config.port)
        };

        let proxy = reqwest::Proxy::http(&proxy_url)
            .map_err(|e| format!("Failed to create proxy: {}", e))?;
        builder = builder.proxy(proxy);
    }

    builder
        .build()
        .map_err(|e| format!("Failed to build client: {}", e))
}

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

pub fn parse_headers(headers_str: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    for line in headers_str.lines() {
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    headers
}

pub fn http_client_request(
    config: &HttpClientConfig,
    method: &str,
    url: &str,
    headers_str: &str,
    body: Option<&str>,
) -> Result<HttpResponse, String> {
    let config = config.clone();
    let method = method.to_string();
    let url = url.to_string();
    let headers_str = headers_str.to_string();
    let body = body.map(|s| s.to_string());

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(http_client_request_async(&config, &method, &url, &headers_str, body.as_deref()))
    }).join().unwrap()
}

async fn http_client_request_async(
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

    for (key, value) in &config.default_headers {
        req_builder = req_builder.header(key, value);
    }

    let request_headers = parse_headers(headers_str);
    for (key, value) in request_headers {
        req_builder = req_builder.header(&key, &value);
    }

    if let Some(body_content) = body {
        req_builder = req_builder.body(body_content.to_string());
    }

    let response = req_builder.send().await.map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status().as_u16();
    let status_text = response
        .status()
        .canonical_reason()
        .unwrap_or("Unknown")
        .to_string();
    let final_url = response.url().to_string();

    let mut response_headers = String::new();
    for (name, value) in response.headers() {
        response_headers.push_str(&format!("{}: {}\n", name, value.to_str().unwrap_or("")));
    }

    let response_body = response.text().await.map_err(|e| format!("Failed to read response: {}", e))?;

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

// HttpClient native methods

#[derive(Debug, Clone)]
pub struct HttpClientState {
    pub base_url: Option<String>,
    pub timeout: u64,
    pub max_redirects: u32,
    pub redirect_policy: String,
    pub proxy_host: Option<String>,
    pub proxy_port: Option<u16>,
    pub verify_ssl: bool,
    pub headers: HashMap<String, String>,
}

impl Default for HttpClientState {
    fn default() -> Self {
        Self {
            base_url: None,
            timeout: 30000,
            max_redirects: 10,
            redirect_policy: "Follow".to_string(),
            proxy_host: None,
            proxy_port: None,
            verify_ssl: true,
            headers: HashMap::new(),
        }
    }
}

impl From<HttpClientState> for HttpClientConfig {
    fn from(state: HttpClientState) -> Self {
        let redirect_policy = match state.redirect_policy.as_str() {
            "Follow" => RedirectPolicy::Follow,
            "None" => RedirectPolicy::None,
            _ => RedirectPolicy::Limited(state.max_redirects),
        };

        HttpClientConfig {
            base_url: state.base_url,
            timeout: state.timeout,
            max_redirects: state.max_redirects,
            redirect_policy,
            proxy: state.proxy_host.map(|host| ProxyConfig {
                host,
                port: state.proxy_port.unwrap_or(8080),
                username: None,
                password: None,
            }),
            verify_ssl: state.verify_ssl,
            default_headers: state.headers,
        }
    }
}

pub fn native_http_client_constructor(_args: &mut Vec<Value>) -> Result<Value, Value> {
    // Constructor is called after native_create, state is already initialized
    Ok(Value::Null)
}

/// Helper function to get or create HttpClientState from instance's native_data
fn get_http_client_state(args: &mut Vec<Value>) -> Result<Arc<Mutex<HttpClientState>>, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "HttpClient method requires instance".to_string(),
        ));
    }

    if let Value::Instance(instance) = &args[0] {
        let instance_lock = instance.lock().unwrap();
        let native_data = instance_lock.native_data.clone();
        drop(instance_lock);

        // Try to get existing state
        let existing = {
            let data = native_data.lock().unwrap();
            if let Some(boxed) = data.as_ref() {
                if let Some(state) = boxed.downcast_ref::<Arc<Mutex<HttpClientState>>>() {
                    Some(state.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(state) = existing {
            return Ok(state);
        }

        // Create new state if not exists
        let state = Arc::new(Mutex::new(HttpClientState::default()));
        let mut data = native_data.lock().unwrap();
        // Store the state in native_data by boxing and casting to dyn Any
        *data = Some(Box::new(state.clone()) as Box<dyn Any + Send + Sync>);
        return Ok(state);
    }

    Err(Value::String("Expected HttpClient instance".to_string()))
}

pub fn native_http_client_set_timeout(args: &mut Vec<Value>) -> Result<Value, Value> {
    let state = get_http_client_state(args)?;
    if args.len() < 2 {
        return Err(Value::String(
            "set_timeout requires timeout argument".to_string(),
        ));
    }

    let timeout = match &args[1] {
        Value::Int64(n) => *n as u64,
        Value::UInt64(n) => *n as u64,
        _ => return Err(Value::String("timeout must be an integer".to_string())),
    };

    let mut state = state.lock().unwrap();
    state.timeout = timeout;
    Ok(Value::Null)
}

pub fn native_http_client_set_base_url(args: &mut Vec<Value>) -> Result<Value, Value> {
    let state = get_http_client_state(args)?;
    if args.len() < 2 {
        return Err(Value::String(
            "set_base_url requires url argument".to_string(),
        ));
    }

    let url = args[1].to_string();
    let mut state = state.lock().unwrap();
    state.base_url = Some(url);
    Ok(Value::Null)
}

pub fn native_http_client_add_header(args: &mut Vec<Value>) -> Result<Value, Value> {
    let state = get_http_client_state(args)?;
    if args.len() < 3 {
        return Err(Value::String(
            "add_header requires key and value arguments".to_string(),
        ));
    }

    let key = args[1].to_string();
    let value = args[2].to_string();

    let mut state = state.lock().unwrap();
    state.headers.insert(key, value);
    Ok(Value::Null)
}

pub fn native_http_client_get(args: &mut Vec<Value>) -> Result<Value, Value> {
    let state = get_http_client_state(args)?;
    if args.len() < 2 {
        return Err(Value::String("get requires URL argument".to_string()));
    }

    let url = args[1].to_string();
    let state_clone = state.lock().unwrap().clone();

    match http_client_request(&state_clone.into(), "GET", &url, "", None) {
        Ok(response) => Ok(Value::String(response.body)),
        Err(e) => Err(Value::String(e)),
    }
}

pub fn native_http_client_post(args: &mut Vec<Value>) -> Result<Value, Value> {
    let state = get_http_client_state(args)?;
    if args.len() < 3 {
        return Err(Value::String(
            "post requires URL and body arguments".to_string(),
        ));
    }

    let url = args[1].to_string();
    let body = args[2].to_string();
    let state_clone = state.lock().unwrap().clone();

    match http_client_request(&state_clone.into(), "POST", &url, "", Some(&body)) {
        Ok(response) => Ok(Value::String(response.body)),
        Err(e) => Err(Value::String(e)),
    }
}

// CamelCase aliases for Bengal API compatibility
pub fn native_http_client_set_timeout_camel(args: &mut Vec<Value>) -> Result<Value, Value> {
    native_http_client_set_timeout(args)
}

pub fn native_http_client_set_base_url_camel(args: &mut Vec<Value>) -> Result<Value, Value> {
    native_http_client_set_base_url(args)
}

pub fn native_http_client_add_header_camel(args: &mut Vec<Value>) -> Result<Value, Value> {
    native_http_client_add_header(args)
}

pub fn native_http_client_get_camel(args: &mut Vec<Value>) -> Result<Value, Value> {
    native_http_client_get(args)
}

pub fn native_http_client_post_camel(args: &mut Vec<Value>) -> Result<Value, Value> {
    native_http_client_post(args)
}

// Additional HttpClient methods
pub fn native_http_client_set_redirect_policy(args: &mut Vec<Value>) -> Result<Value, Value> {
    let state = get_http_client_state(args)?;
    if args.len() < 2 {
        return Err(Value::String(
            "set_redirect_policy requires policy argument".to_string(),
        ));
    }

    let policy = match &args[1] {
        Value::Int64(n) => *n as i32,
        Value::UInt64(n) => *n as i32,
        Value::Int32(n) => *n,
        _ => return Err(Value::String("policy must be an integer".to_string())),
    };

    let mut state = state.lock().unwrap();
    state.redirect_policy = match policy {
        0 => "Follow".to_string(),
        1 => "Limited".to_string(),
        2 => "None".to_string(),
        _ => "Follow".to_string(),
    };
    Ok(Value::Null)
}

pub fn native_http_client_set_max_redirects(args: &mut Vec<Value>) -> Result<Value, Value> {
    let state = get_http_client_state(args)?;
    if args.len() < 2 {
        return Err(Value::String(
            "set_max_redirects requires max argument".to_string(),
        ));
    }

    let max = match &args[1] {
        Value::Int64(n) => *n as u32,
        Value::UInt64(n) => *n as u32,
        Value::Int32(n) => *n as u32,
        Value::UInt32(n) => *n,
        _ => return Err(Value::String("max must be an integer".to_string())),
    };

    let mut state = state.lock().unwrap();
    state.max_redirects = max;
    Ok(Value::Null)
}

pub fn native_http_client_set_proxy(args: &mut Vec<Value>) -> Result<Value, Value> {
    let state = get_http_client_state(args)?;
    if args.len() < 3 {
        return Err(Value::String(
            "set_proxy requires host and port arguments".to_string(),
        ));
    }

    let host = args[1].to_string();
    let port = match &args[2] {
        Value::Int64(n) => *n as u16,
        Value::UInt64(n) => *n as u16,
        Value::Int32(n) => *n as u16,
        Value::UInt32(n) => *n as u16,
        _ => return Err(Value::String("port must be an integer".to_string())),
    };

    let mut state = state.lock().unwrap();
    state.proxy_host = Some(host);
    state.proxy_port = Some(port);
    Ok(Value::Null)
}

pub fn native_http_client_set_verify_ssl(args: &mut Vec<Value>) -> Result<Value, Value> {
    let state = get_http_client_state(args)?;
    if args.len() < 2 {
        return Err(Value::String(
            "set_verify_ssl requires verify argument".to_string(),
        ));
    }

    let verify = match &args[1] {
        Value::Bool(b) => *b,
        _ => return Err(Value::String("verify must be a boolean".to_string())),
    };

    let mut state = state.lock().unwrap();
    state.verify_ssl = verify;
    Ok(Value::Null)
}

// CamelCase aliases for additional methods
pub fn native_http_client_set_redirect_policy_camel(args: &mut Vec<Value>) -> Result<Value, Value> {
    native_http_client_set_redirect_policy(args)
}

pub fn native_http_client_set_max_redirects_camel(args: &mut Vec<Value>) -> Result<Value, Value> {
    native_http_client_set_max_redirects(args)
}

pub fn native_http_client_set_proxy_camel(args: &mut Vec<Value>) -> Result<Value, Value> {
    native_http_client_set_proxy(args)
}

pub fn native_http_client_set_verify_ssl_camel(args: &mut Vec<Value>) -> Result<Value, Value> {
    native_http_client_set_verify_ssl(args)
}

// HttpClient native_create callback
pub fn native_http_client_native_create(args: &mut Vec<Value>) -> Result<Value, Value> {
    // Initialize the HttpClient instance with default state
    if args.is_empty() {
        return Err(Value::String(
            "HttpClient constructor requires instance".to_string(),
        ));
    }

    if let Value::Instance(instance) = &args[0] {
        let state = Arc::new(Mutex::new(HttpClientState::default()));
        let instance_lock = instance.lock().unwrap();
        let mut native_data = instance_lock.native_data.lock().unwrap();
        *native_data = Some(Box::new(state) as Box<dyn Any + Send + Sync>);
        drop(native_data);
        drop(instance_lock);
        Ok(Value::Null)
    } else {
        Err(Value::String("Expected HttpClient instance".to_string()))
    }
}
