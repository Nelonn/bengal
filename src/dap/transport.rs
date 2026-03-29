//! TCP Transport for DAP
//! 
//! Implements TCP-based transport for Debug Adapter Protocol communication.
//! Uses the standard DAP wire protocol: Content-Length header followed by JSON body.

use crate::dap::types::*;
use serde_json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader, AsyncReadExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

/// DAP message header
const CONTENT_LENGTH: &str = "Content-Length";

/// Error type for transport operations
#[derive(Debug)]
pub enum TransportError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidHeader(String),
    ConnectionClosed,
    ChannelClosed,
}

impl From<std::io::Error> for TransportError {
    fn from(err: std::io::Error) -> Self {
        TransportError::Io(err)
    }
}

impl From<serde_json::Error> for TransportError {
    fn from(err: serde_json::Error) -> Self {
        TransportError::Json(err)
    }
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::Io(e) => write!(f, "IO error: {}", e),
            TransportError::Json(e) => write!(f, "JSON error: {}", e),
            TransportError::InvalidHeader(h) => write!(f, "Invalid header: {}", h),
            TransportError::ConnectionClosed => write!(f, "Connection closed"),
            TransportError::ChannelClosed => write!(f, "Channel closed"),
        }
    }
}

impl std::error::Error for TransportError {}

pub type Result<T> = std::result::Result<T, TransportError>;

/// TCP Transport for DAP communication
pub struct DapTransport {
    stream: TcpStream,
}

impl DapTransport {
    /// Create a new transport from a TCP stream
    pub fn new(stream: TcpStream) -> Self {
        Self { stream }
    }

    /// Get the underlying stream
    pub fn into_inner(self) -> TcpStream {
        self.stream
    }

    /// Read a DAP message from the stream
    pub async fn read_message(&mut self) -> Result<ProtocolMessage> {
        let (reader, _) = self.stream.split();
        let mut buf_reader = TokioBufReader::new(reader);
        
        // Read headers
        let mut content_length: Option<usize> = None;
        let mut line = String::new();
        
        loop {
            line.clear();
            let bytes_read = buf_reader.read_line(&mut line).await?;
            
            if bytes_read == 0 {
                return Err(TransportError::ConnectionClosed);
            }
            
            let trimmed = line.trim();
            
            // Empty line marks end of headers
            if trimmed.is_empty() {
                break;
            }
            
            // Parse Content-Length header
            if let Some(rest) = trimmed.strip_prefix(CONTENT_LENGTH) {
                if let Some(value_str) = rest.strip_prefix(':') {
                    let value_str = value_str.trim();
                    content_length = Some(value_str.parse().map_err(|_| {
                        TransportError::InvalidHeader(format!("Invalid Content-Length: {}", value_str))
                    })?);
                }
            }
        }
        
        let content_length = content_length.ok_or_else(|| {
            TransportError::InvalidHeader("Missing Content-Length header".to_string())
        })?;
        
        // Read content
        let mut content = vec![0u8; content_length];
        buf_reader.read_exact(&mut content).await?;
        
        // Parse JSON
        let message: ProtocolMessage = serde_json::from_slice(&content)?;
        Ok(message)
    }

    /// Write a DAP message to the stream
    pub async fn write_message(&mut self, message: &ProtocolMessage) -> Result<()> {
        let json = serde_json::to_string(message)?;
        let content_length = json.len();
        
        let (_, mut writer) = self.stream.split();
        
        // Write headers
        let header = format!("Content-Length: {}\r\n\r\n", content_length);
        writer.write_all(header.as_bytes()).await?;
        
        // Write content
        writer.write_all(json.as_bytes()).await?;
        writer.flush().await?;
        
        Ok(())
    }

    /// Send a response message
    pub async fn send_response(&mut self, response: Response) -> Result<()> {
        let message = ProtocolMessage::Response(response);
        self.write_message(&message).await
    }

    /// Send an event message
    pub async fn send_event(&mut self, event: Event) -> Result<()> {
        let message = ProtocolMessage::Event(event);
        self.write_message(&message).await
    }
}

/// Message channel for communication between transport and session
pub struct MessageChannel {
    pub tx: mpsc::Sender<ProtocolMessage>,
    pub rx: mpsc::Receiver<ProtocolMessage>,
}

impl MessageChannel {
    pub fn new(buffer_size: usize) -> Self {
        let (tx, rx) = mpsc::channel(buffer_size);
        Self { tx, rx }
    }
}

/// Transport handle that runs in a separate task
pub struct TransportHandle {
    pub incoming_tx: mpsc::Sender<ProtocolMessage>,
    pub outgoing_rx: mpsc::Receiver<ProtocolMessage>,
}

/// Start a TCP server for DAP communication
pub async fn start_tcp_server(
    host: &str,
    port: u16,
) -> std::io::Result<(TcpStream, std::net::SocketAddr)> {
    use tokio::net::TcpListener;
    
    let addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&addr).await?;
    println!("DAP server listening on {}", addr);
    
    // Accept a single connection for now
    let (stream, addr) = listener.accept().await?;
    println!("Client connected from {}", addr);
    
    Ok((stream, addr))
}

/// Connect to a DAP server as a client
pub async fn connect_tcp_client(
    host: &str,
    port: u16,
) -> std::io::Result<TcpStream> {
    let addr = format!("{}:{}", host, port);
    let stream = TcpStream::connect(&addr).await?;
    println!("Connected to DAP server at {}", addr);
    Ok(stream)
}

/// Read messages from transport in a loop and send to channel
pub async fn read_loop(mut transport: DapTransport, tx: mpsc::Sender<ProtocolMessage>) {
    loop {
        match transport.read_message().await {
            Ok(message) => {
                if tx.send(message).await.is_err() {
                    break;
                }
            }
            Err(e) => {
                eprintln!("Transport read error: {}", e);
                break;
            }
        }
    }
}

/// Read messages from read half in a loop and send to channel
pub async fn read_loop_half(read_half: tokio::net::tcp::OwnedReadHalf, tx: mpsc::Sender<ProtocolMessage>) {
    use tokio::io::AsyncBufReadExt;
    let mut buf_reader = TokioBufReader::new(read_half);
    let mut content_length: Option<usize> = None;
    let mut line = String::new();
    
    loop {
        line.clear();
        match buf_reader.read_line(&mut line).await {
            Ok(0) => break, // Connection closed
            Ok(_) => {
                let trimmed = line.trim();
                
                // Empty line marks end of headers
                if trimmed.is_empty() {
                    if let Some(len) = content_length {
                        // Read content
                        let mut content = vec![0u8; len];
                        match buf_reader.read_exact(&mut content).await {
                            Ok(_) => {
                                // Parse JSON
                                match serde_json::from_slice::<ProtocolMessage>(&content) {
                                    Ok(message) => {
                                        if tx.send(message).await.is_err() {
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("JSON parse error: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Transport read error: {}", e);
                                break;
                            }
                        }
                        content_length = None;
                    }
                } else if let Some(rest) = trimmed.strip_prefix(CONTENT_LENGTH) {
                    if let Some(value_str) = rest.strip_prefix(':') {
                        let value_str = value_str.trim();
                        content_length = Some(value_str.parse().unwrap_or(0));
                    }
                }
            }
            Err(e) => {
                eprintln!("Transport read error: {}", e);
                break;
            }
        }
    }
}

/// Write messages from channel to transport
pub async fn write_loop(mut transport: DapTransport, mut rx: mpsc::Receiver<ProtocolMessage>) {
    while let Some(message) = rx.recv().await {
        if let Err(e) = transport.write_message(&message).await {
            eprintln!("Transport write error: {}", e);
            break;
        }
    }
}

/// Write messages from channel to write half
pub async fn write_loop_half(
    write_half: tokio::net::tcp::OwnedWriteHalf,
    mut rx: mpsc::Receiver<ProtocolMessage>,
) {
    use tokio::io::AsyncWriteExt;
    let mut writer = write_half;
    
    while let Some(message) = rx.recv().await {
        match serde_json::to_string(&message) {
            Ok(json) => {
                let content_length = json.len();
                let header = format!("Content-Length: {}\r\n\r\n", content_length);
                
                if let Err(e) = writer.write_all(header.as_bytes()).await {
                    eprintln!("Transport write error: {}", e);
                    break;
                }
                if let Err(e) = writer.write_all(json.as_bytes()).await {
                    eprintln!("Transport write error: {}", e);
                    break;
                }
                if let Err(e) = writer.flush().await {
                    eprintln!("Transport flush error: {}", e);
                    break;
                }
            }
            Err(e) => {
                eprintln!("JSON serialize error: {}", e);
            }
        }
    }
}
