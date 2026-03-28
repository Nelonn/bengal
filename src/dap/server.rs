//! DAP Server Implementation
//! 
//! Runs the DAP server and handles the debug session loop.

use crate::dap::{
    DebugSession,
    types::*,
};
use crate::dap::transport::start_tcp_server;
use tokio::sync::mpsc;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Shared state for communication between server loops
struct ServerState {
    outgoing_tx: mpsc::Sender<ProtocolMessage>,
    session: DebugSession,
}

impl ServerState {
    fn new(outgoing_tx: mpsc::Sender<ProtocolMessage>) -> Self {
        Self {
            outgoing_tx,
            session: DebugSession::new(),
        }
    }

    async fn send_response(&self, response: Response) {
        let _ = self.outgoing_tx.send(ProtocolMessage::Response(response)).await;
    }

    async fn send_event(&self, event: Event) {
        let _ = self.outgoing_tx.send(ProtocolMessage::Event(event)).await;
    }
}

/// Run the DAP server with a source file
pub async fn run_dap_server_with_source(
    host: &str,
    port: u16,
    source_file: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let (stream, addr) = start_tcp_server(host, port).await?;
    
    // Create channels for message passing
    let (incoming_tx, mut incoming_rx) = mpsc::channel::<ProtocolMessage>(100);
    let (outgoing_tx, outgoing_rx) = mpsc::channel::<ProtocolMessage>(100);
    
    // Wrap outgoing_tx for sharing
    let outgoing_tx = Arc::new(outgoing_tx);
    
    // Split the stream into read and write halves
    let (read_half, write_half) = stream.into_split();
    
    // Spawn read loop
    let read_tx = incoming_tx.clone();
    tokio::spawn(async move {
        crate::dap::transport::read_loop_half(read_half, read_tx).await;
    });

    // Spawn write loop
    let write_outgoing_rx = outgoing_rx;
    tokio::spawn(async move {
        crate::dap::transport::write_loop_half(write_half, write_outgoing_rx).await;
    });

    println!("DAP session started");

    // Create shared state
    let state = Arc::new(Mutex::new(ServerState::new((*outgoing_tx).clone())));
    
    // Pre-load source file
    {
        let mut state_guard = state.lock().await;
        if let Err(e) = state_guard.session.load_source_file(source_file).await {
            eprintln!("Failed to load source file: {}", e);
            let response = Response {
                seq: state_guard.session.next_seq(),
                message_type: "response".to_string(),
                request_seq: 0,
                success: false,
                command: "launch".to_string(),
                message: Some(format!("Failed to load source: {}", e)),
                body: None,
            };
            state_guard.send_response(response).await;
            return Ok(());
        }
    }

    // Main message processing loop
    loop {
        tokio::select! {
            // Handle incoming DAP messages
            Some(message) = incoming_rx.recv() => {
                match message {
                    ProtocolMessage::Request(request) => {
                        let mut state_guard = state.lock().await;
                        handle_request(&mut state_guard, request).await;
                    }
                    ProtocolMessage::Response(_) => {
                        // We don't expect responses from the client
                    }
                    ProtocolMessage::Event(_) => {
                        // We don't expect events from the client
                    }
                }
            }
        }
    }
}

/// Handle a DAP request
async fn handle_request(
    state: &mut ServerState,
    request: Request,
) {
    println!("DAP Request: {} (seq={})", request.command, request.seq);

    let command = request.command.clone();
    let response = match request.command.as_str() {
        "initialize" => {
            let args: InitializeRequestArguments = match serde_json::from_value(request.arguments) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Failed to parse initialize args: {}", e);
                    InitializeRequestArguments::default()
                }
            };
            state.session.handle_initialize(request.seq, args).await
        }

        "launch" => {
            let args: LaunchRequestArguments = match serde_json::from_value(request.arguments) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Failed to parse launch args: {}", e);
                    LaunchRequestArguments::default()
                }
            };
            
            let response = state.session.handle_launch(request.seq, args).await;
            
            // Send initialized event after successful launch
            if let Ok(ref resp) = response {
                if resp.success {
                    let initialized_event = Event {
                        seq: state.session.next_seq(),
                        message_type: "event".to_string(),
                        event: "initialized".to_string(),
                        body: Some(serde_json::json!({})),
                    };
                    state.send_event(initialized_event).await;
                }
            }
            
            response
        }

        "attach" => {
            let args: AttachRequestArguments = match serde_json::from_value(request.arguments) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Failed to parse attach args: {}", e);
                    AttachRequestArguments::default()
                }
            };
            state.session.handle_attach(request.seq, args).await
        }

        "configurationDone" => {
            state.session.handle_configuration_done(request.seq).await
        }

        "setBreakpoints" => {
            let args: SetBreakpointsArguments = match serde_json::from_value(request.arguments) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Failed to parse setBreakpoints args: {}", e);
                    return;
                }
            };
            state.session.handle_set_breakpoints(args, request.seq).await
        }

        "threads" => {
            state.session.handle_threads(request.seq).await
        }

        "stackTrace" => {
            let args: StackTraceArguments = match serde_json::from_value(request.arguments) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Failed to parse stackTrace args: {}", e);
                    return;
                }
            };
            state.session.handle_stack_trace(args, request.seq).await
        }

        "scopes" => {
            let args: ScopesArguments = match serde_json::from_value(request.arguments) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Failed to parse scopes args: {}", e);
                    return;
                }
            };
            state.session.handle_scopes(args, request.seq).await
        }

        "variables" => {
            let args: VariablesArguments = match serde_json::from_value(request.arguments) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Failed to parse variables args: {}", e);
                    return;
                }
            };
            state.session.handle_variables(args, request.seq).await
        }

        "continue" => {
            let args: ContinueArguments = match serde_json::from_value(request.arguments) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Failed to parse continue args: {}", e);
                    return;
                }
            };
            state.session.handle_continue(args, request.seq).await
        }

        "next" => {
            let args: NextArguments = match serde_json::from_value(request.arguments) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Failed to parse next args: {}", e);
                    return;
                }
            };
            state.session.handle_next(args, request.seq).await
        }

        "stepIn" => {
            let args: StepInArguments = match serde_json::from_value(request.arguments) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Failed to parse stepIn args: {}", e);
                    return;
                }
            };
            state.session.handle_step_in(args, request.seq).await
        }

        "stepOut" => {
            let args: StepOutArguments = match serde_json::from_value(request.arguments) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Failed to parse stepOut args: {}", e);
                    return;
                }
            };
            state.session.handle_step_out(args, request.seq).await
        }

        "evaluate" => {
            let args: EvaluateArguments = match serde_json::from_value(request.arguments) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Failed to parse evaluate args: {}", e);
                    return;
                }
            };
            state.session.handle_evaluate(args, request.seq).await
        }

        "disconnect" => {
            let args: DisconnectArguments = match serde_json::from_value(request.arguments) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Failed to parse disconnect args: {}", e);
                    DisconnectArguments::default()
                }
            };
            state.session.handle_disconnect(args, request.seq).await
        }

        _ => {
            Ok(Response {
                seq: state.session.next_seq(),
                message_type: "response".to_string(),
                request_seq: request.seq,
                success: false,
                command: command.clone(),
                message: Some(format!("Unknown command: {}", command)),
                body: None,
            })
        }
    };

    if let Ok(resp) = response {
        state.send_response(resp).await;
    }
}
