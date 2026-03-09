use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use sparkler::{Value, PromiseState};

pub fn native_http_get(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String("http_get requires URL argument".to_string()));
    }
    let url = args[0].to_string();
    
    let promise = Arc::new(TokioMutex::new(PromiseState::Pending));
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

pub fn native_http_post(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.len() < 2 {
        return Err(Value::String("http_post requires URL and body arguments".to_string()));
    }
    let url = args[0].to_string();
    let body = args[1].to_string();
    
    let promise = Arc::new(TokioMutex::new(PromiseState::Pending));
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

pub fn build_client(config: &HttpClientConfig) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(config.timeout))
        .danger_accept_invalid_certs(!config.verify_ssl);
    
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
    
    let response = req_builder.send().await
        .map_err(|e| format!("Request failed: {}", e))?;
    
    let status = response.status().as_u16();
    let status_text = response.status().canonical_reason().unwrap_or("Unknown").to_string();
    let final_url = response.url().to_string();
    
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
