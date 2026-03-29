//! DAP Session Manager
//! 
//! Handles DAP requests and manages debug session state.

use crate::dap::types::*;
use crate::dap::transport::TransportError;
use sparkler::{VM, Value};
use bengal_compiler::{HlirCompiler, CompilerOptions, sparkler_to_bytecode};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Debug session state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugState {
    /// No program loaded
    Uninitialized,
    /// Program loaded, ready to run
    Ready,
    /// Program running
    Running,
    /// Program paused at breakpoint or step
    Paused,
    /// Program finished
    Terminated,
}

/// Information about a loaded source file
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: String,
    pub content: String,
    pub source_reference: i64,
}

/// Variable container for DAP
#[derive(Clone)]
pub enum VariableContainer {
    /// Global or local scope
    Scope(HashMap<String, Value>),
    /// Array elements
    Array(Arc<Mutex<Vec<Value>>>),
    /// Instance fields
    Instance(Arc<Mutex<sparkler::vm::Instance>>),
}

/// DAP Debug Session
pub struct DebugSession {
    /// Current debug state
    state: DebugState,
    /// The VM being debugged
    vm: Option<VM>,
    /// Loaded source files
    pub sources: HashMap<i64, SourceFile>,
    /// Next source reference ID
    next_source_ref: i64,
    /// Breakpoint counter
    next_breakpoint_id: i64,
    /// Current source file being debugged
    current_source: Option<String>,
    /// Sequence counter for messages
    sequence: u32,
    /// Client capabilities
    client_capabilities: Option<Capabilities>,
    /// Configuration done flag
    configuration_done: bool,
    /// Stop on entry flag
    stop_on_entry: bool,
    /// Current frame ID for stack traces
    frame_counter: i64,
}

impl DebugSession {
    /// Create a new debug session
    pub fn new() -> Self {
        Self {
            state: DebugState::Uninitialized,
            vm: None,
            sources: HashMap::new(),
            next_source_ref: 1,
            next_breakpoint_id: 1,
            current_source: None,
            sequence: 0,
            client_capabilities: None,
            configuration_done: false,
            stop_on_entry: false,
            frame_counter: 0,
        }
    }

    /// Get next sequence number
    pub fn next_seq(&mut self) -> u32 {
        self.sequence += 1;
        self.sequence
    }

    /// Get next source reference
    pub fn next_source_ref(&mut self) -> i64 {
        let ref_id = self.next_source_ref;
        self.next_source_ref += 1;
        ref_id
    }

    /// Get next breakpoint ID
    fn next_breakpoint_id(&mut self) -> i64 {
        let id = self.next_breakpoint_id;
        self.next_breakpoint_id += 1;
        id
    }

    /// Get next frame ID
    fn next_frame_id(&mut self) -> i64 {
        self.frame_counter += 1;
        self.frame_counter
    }

    // ========================================================================
    // Request Handlers
    // ========================================================================

    /// Handle initialize request
    pub async fn handle_initialize(
        &mut self,
        request_seq: u32,
        args: InitializeRequestArguments,
    ) -> Result<Response, TransportError> {
        // Store client capabilities
        self.client_capabilities = Some(Capabilities {
            supports_configuration_done_request: Some(true),
            supports_function_breakpoints: Some(true),
            supports_conditional_breakpoints: Some(true),
            supports_hit_conditional_breakpoints: Some(true),
            supports_evaluate_for_hovers: Some(true),
            supports_step_back: Some(false),
            supports_set_variable: Some(false),
            supports_restart_frame: Some(false),
            supports_log_points: Some(true),
            ..Default::default()
        });

        let capabilities = serde_json::to_value(&self.client_capabilities).unwrap();

        Ok(Response {
            seq: self.next_seq(),
            message_type: "response".to_string(),
            request_seq,
            success: true,
            command: "initialize".to_string(),
            message: None,
            body: Some(capabilities),
        })
    }

    /// Handle launch request
    pub async fn handle_launch(
        &mut self,
        request_seq: u32,
        args: LaunchRequestArguments,
    ) -> Result<Response, TransportError> {
        let source_file = args.source_file
            .or(args.program)
            .ok_or_else(|| TransportError::InvalidHeader("No source file specified".to_string()))?;

        self.current_source = Some(source_file.clone());
        self.stop_on_entry = args.stop_on_entry.unwrap_or(false);

        // Load and compile the source file
        match self.load_source_file(&source_file).await {
            Ok(_) => {
                self.state = DebugState::Ready;
                
                Ok(Response {
                    seq: self.next_seq(),
                    message_type: "response".to_string(),
                    request_seq,
                    success: true,
                    command: "launch".to_string(),
                    message: None,
                    body: None,
                })
            }
            Err(e) => {
                self.state = DebugState::Terminated;
                
                Ok(Response {
                    seq: self.next_seq(),
                    message_type: "response".to_string(),
                    request_seq,
                    success: false,
                    command: "launch".to_string(),
                    message: Some(format!("Failed to load source: {}", e)),
                    body: None,
                })
            }
        }
    }

    /// Handle attach request
    pub async fn handle_attach(
        &mut self,
        request_seq: u32,
        args: AttachRequestArguments,
    ) -> Result<Response, TransportError> {
        // For now, attach is similar to launch - we could support process attachment later
        self.handle_launch(request_seq, LaunchRequestArguments {
            source_file: args.host.map(|h| format!("{}:{}", h, args.port.unwrap_or(0))),
            ..Default::default()
        }).await
    }

    /// Handle configurationDone request
    pub async fn handle_configuration_done(
        &mut self,
        request_seq: u32,
    ) -> Result<Response, TransportError> {
        self.configuration_done = true;
        
        let response = Response {
            seq: self.next_seq(),
            message_type: "response".to_string(),
            request_seq,
            success: true,
            command: "configurationDone".to_string(),
            message: None,
            body: None,
        };

        // If stop on entry is set, start paused
        if self.stop_on_entry && self.state == DebugState::Ready {
            self.state = DebugState::Paused;
        }

        Ok(response)
    }

    /// Handle setBreakpoints request
    pub async fn handle_set_breakpoints(
        &mut self,
        args: SetBreakpointsArguments,
        request_seq: u32,
    ) -> Result<Response, TransportError> {
        let mut breakpoints = Vec::new();
        
        if let Some(source_path) = &args.source.path {
            // Clear existing breakpoints for this source
            if let Some(vm) = &mut self.vm {
                vm.breakpoints.retain(|(file, _)| file != source_path);
            }

            // Set new breakpoints
            if let Some(bps) = &args.breakpoints {
                for bp in bps {
                    if let Some(vm) = &mut self.vm {
                        let _ = vm.set_breakpoint(source_path, bp.line as usize);
                    }

                    breakpoints.push(Breakpoint {
                        id: Some(self.next_breakpoint_id()),
                        verified: true,
                        message: None,
                        source: Some(Source {
                            name: args.source.name.clone(),
                            path: Some(source_path.clone()),
                            source_reference: args.source.source_reference,
                            ..Default::default()
                        }),
                        line: Some(bp.line),
                        column: bp.column,
                        end_line: None,
                        end_column: None,
                        instruction_reference: None,
                        offset: None,
                    });
                }
            }
        }

        let body = SetBreakpointsResponse { breakpoints };
        let body_json = serde_json::to_value(&body).unwrap();

        Ok(Response {
            seq: self.next_seq(),
            message_type: "response".to_string(),
            request_seq,
            success: true,
            command: "setBreakpoints".to_string(),
            message: None,
            body: Some(body_json),
        })
    }

    /// Handle threads request
    pub async fn handle_threads(
        &mut self,
        request_seq: u32,
    ) -> Result<Response, TransportError> {
        // Bengal has a single main thread
        let threads = vec![Thread {
            id: 1,
            name: "Main Thread".to_string(),
        }];

        let body = ThreadsResponse { threads };
        let body_json = serde_json::to_value(&body).unwrap();

        Ok(Response {
            seq: self.next_seq(),
            message_type: "response".to_string(),
            request_seq,
            success: true,
            command: "threads".to_string(),
            message: None,
            body: Some(body_json),
        })
    }

    /// Handle stackTrace request
    pub async fn handle_stack_trace(
        &mut self,
        args: StackTraceArguments,
        request_seq: u32,
    ) -> Result<Response, TransportError> {
        let mut stack_frames = Vec::new();
        let mut frame_data = Vec::new();

        if let Some(vm) = &self.vm {
            let call_stack = &vm.context.call_stack;
            
            for frame in call_stack.iter() {
                frame_data.push((
                    frame.function_name.clone(),
                    frame.source_file.clone(),
                    frame.line_number,
                ));
            }
        }

        for (func_name, source_file, line_number) in frame_data {
            let frame_id = self.next_frame_id();
            
            stack_frames.push(StackFrame {
                id: frame_id,
                name: func_name,
                source: source_file.as_ref().map(|path: &String| Source {
                    name: std::path::Path::new(path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string()),
                    path: Some(path.clone()),
                    ..Default::default()
                }),
                line: line_number as u32,
                column: 0,
                end_line: None,
                end_column: None,
                can_restart: Some(false),
                instruction_pointer_reference: None,
                module_id: None,
                presentation_hint: None,
            });
        }

        // If no frames, add a synthetic one
        if stack_frames.is_empty() {
            stack_frames.push(StackFrame {
                id: self.next_frame_id(),
                name: "<entry>".to_string(),
                source: self.current_source.as_ref().map(|path| Source {
                    name: std::path::Path::new(path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string()),
                    path: Some(path.clone()),
                    ..Default::default()
                }),
                line: 0,
                column: 0,
                end_line: None,
                end_column: None,
                can_restart: Some(false),
                instruction_pointer_reference: None,
                module_id: None,
                presentation_hint: None,
            });
        }

        let total_frames = stack_frames.len() as u32;
        
        // Apply pagination
        let start = args.start_frame.unwrap_or(0) as usize;
        let levels = args.levels.unwrap_or(total_frames) as usize;
        let end = (start + levels).min(stack_frames.len());
        stack_frames = stack_frames[start..end].to_vec();

        let body = StackTraceResponse {
            stack_frames,
            total_frames: Some(total_frames),
        };
        let body_json = serde_json::to_value(&body).unwrap();

        Ok(Response {
            seq: self.next_seq(),
            message_type: "response".to_string(),
            request_seq,
            success: true,
            command: "stackTrace".to_string(),
            message: None,
            body: Some(body_json),
        })
    }

    /// Handle scopes request
    pub async fn handle_scopes(
        &mut self,
        args: ScopesArguments,
        request_seq: u32,
    ) -> Result<Response, TransportError> {
        // Create Locals and Globals scopes
        let mut scopes = Vec::new();

        if let Some(vm) = &self.vm {
            // Locals scope
            scopes.push(Scope {
                name: "Locals".to_string(),
                presentation_hint: Some("locals".to_string()),
                variables_reference: 1, // Frame-local scope
                named_variables: None,
                indexed_variables: None,
                expensive: Some(false),
                source: None,
                line: None,
                column: None,
                end_line: None,
                end_column: None,
            });

            // Globals scope
            scopes.push(Scope {
                name: "Globals".to_string(),
                presentation_hint: Some("globals".to_string()),
                variables_reference: 2, // Global scope
                named_variables: None,
                indexed_variables: None,
                expensive: Some(false),
                source: None,
                line: None,
                column: None,
                end_line: None,
                end_column: None,
            });
        } else {
            // Empty scopes if no VM
            scopes.push(Scope {
                name: "Locals".to_string(),
                presentation_hint: Some("locals".to_string()),
                variables_reference: 0,
                named_variables: Some(0),
                indexed_variables: Some(0),
                expensive: Some(false),
                source: None,
                line: None,
                column: None,
                end_line: None,
                end_column: None,
            });
        }

        let body = ScopesResponse { scopes };
        let body_json = serde_json::to_value(&body).unwrap();

        Ok(Response {
            seq: self.next_seq(),
            message_type: "response".to_string(),
            request_seq,
            success: true,
            command: "scopes".to_string(),
            message: None,
            body: Some(body_json),
        })
    }

    /// Handle variables request
    pub async fn handle_variables(
        &mut self,
        args: VariablesArguments,
        request_seq: u32,
    ) -> Result<Response, TransportError> {
        let mut variables = Vec::new();

        if let Some(vm) = &self.vm {
            match args.variables_reference {
                1 => {
                    // Local variables - for now return empty since we don't have easy access to frame locals
                    // Would need to implement proper variable inspection
                }
                2 => {
                    // Global variables
                    for (name, value) in &vm.context.locals {
                        variables.push(value_to_variable(name, value, 0));
                    }
                }
                _ => {
                    // Container reference - would need more complex handling for nested structures
                }
            }
        }

        let body = VariablesResponse { variables };
        let body_json = serde_json::to_value(&body).unwrap();

        Ok(Response {
            seq: self.next_seq(),
            message_type: "response".to_string(),
            request_seq,
            success: true,
            command: "variables".to_string(),
            message: None,
            body: Some(body_json),
        })
    }

    /// Handle continue request
    pub async fn handle_continue(
        &mut self,
        args: ContinueArguments,
        request_seq: u32,
    ) -> Result<Response, TransportError> {
        self.state = DebugState::Running;

        let body = ContinueResponse {
            all_threads_continued: Some(true),
        };
        let body_json = serde_json::to_value(&body).unwrap();

        Ok(Response {
            seq: self.next_seq(),
            message_type: "response".to_string(),
            request_seq,
            success: true,
            command: "continue".to_string(),
            message: None,
            body: Some(body_json),
        })
    }

    /// Handle next (step over) request
    pub async fn handle_next(
        &mut self,
        args: NextArguments,
        request_seq: u32,
    ) -> Result<Response, TransportError> {
        // For now, next is similar to continue - would need line-level stepping support
        self.state = DebugState::Running;

        let body = ContinueResponse {
            all_threads_continued: Some(true),
        };
        let body_json = serde_json::to_value(&body).unwrap();

        Ok(Response {
            seq: self.next_seq(),
            message_type: "response".to_string(),
            request_seq,
            success: true,
            command: "next".to_string(),
            message: None,
            body: Some(body_json),
        })
    }

    /// Handle stepIn request
    pub async fn handle_step_in(
        &mut self,
        args: StepInArguments,
        request_seq: u32,
    ) -> Result<Response, TransportError> {
        self.state = DebugState::Running;

        let body = ContinueResponse {
            all_threads_continued: Some(true),
        };
        let body_json = serde_json::to_value(&body).unwrap();

        Ok(Response {
            seq: self.next_seq(),
            message_type: "response".to_string(),
            request_seq,
            success: true,
            command: "stepIn".to_string(),
            message: None,
            body: Some(body_json),
        })
    }

    /// Handle stepOut request
    pub async fn handle_step_out(
        &mut self,
        args: StepOutArguments,
        request_seq: u32,
    ) -> Result<Response, TransportError> {
        self.state = DebugState::Running;

        let body = ContinueResponse {
            all_threads_continued: Some(true),
        };
        let body_json = serde_json::to_value(&body).unwrap();

        Ok(Response {
            seq: self.next_seq(),
            message_type: "response".to_string(),
            request_seq,
            success: true,
            command: "stepOut".to_string(),
            message: None,
            body: Some(body_json),
        })
    }

    /// Handle evaluate request
    pub async fn handle_evaluate(
        &mut self,
        args: EvaluateArguments,
        request_seq: u32,
    ) -> Result<Response, TransportError> {
        // For now, return a placeholder response
        // Full implementation would need expression evaluation in current context
        let body = EvaluateResponse {
            result: format!("<evaluate: {}>", args.expression),
            variable_type: None,
            variables_reference: 0,
            named_variables: None,
            indexed_variables: None,
            memory_reference: None,
            presentation_hint: None,
        };
        let body_json = serde_json::to_value(&body).unwrap();

        Ok(Response {
            seq: self.next_seq(),
            message_type: "response".to_string(),
            request_seq,
            success: true,
            command: "evaluate".to_string(),
            message: None,
            body: Some(body_json),
        })
    }

    /// Handle disconnect request
    pub async fn handle_disconnect(
        &mut self,
        args: DisconnectArguments,
        request_seq: u32,
    ) -> Result<Response, TransportError> {
        self.state = DebugState::Terminated;

        Ok(Response {
            seq: self.next_seq(),
            message_type: "response".to_string(),
            request_seq,
            success: true,
            command: "disconnect".to_string(),
            message: None,
            body: None,
        })
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Load a source file, compile it, and initialize the VM
    pub async fn load_source_file(&mut self, path: &str) -> Result<(), String> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        // Store source for later retrieval
        let source_ref = self.next_source_ref();
        self.sources.insert(source_ref, SourceFile {
            path: path.to_string(),
            content: source.clone(),
            source_reference: source_ref,
        });

        // Compile the source
        let options = CompilerOptions {
            enable_type_checking: true,
            search_paths: vec!["std".to_string()],
            emit_llvm_ir: false,
            emit_sparkler_bytecode: true,
        };

        let mut compiler = HlirCompiler::with_path_and_options(&source, path, options);
        let result = compiler.compile()
            .map_err(|e| format!("Compilation error: {}", e))?;

        let _bytecode = sparkler_to_bytecode(
            result.sparkler_bytecode
                .ok_or("Bytecode generation failed")?
        );

        // Create VM
        let mut vm = VM::new();
        bengal_std::register_all(&mut vm);

        // Enable debugging
        vm.is_debugging = true;

        // Store VM
        self.vm = Some(vm);

        Ok(())
    }

    /// Send a stopped event
    pub async fn send_stopped_event(
        &mut self,
        reason: StoppedReason,
        thread_id: Option<i64>,
    ) -> Option<Event> {
        Some(Event {
            seq: self.next_seq(),
            message_type: "event".to_string(),
            event: "stopped".to_string(),
            body: serde_json::to_value(&StoppedEventBody {
                reason,
                description: None,
                thread_id,
                preserve_focus_hint: None,
                text: None,
                all_threads_stopped: Some(true),
                hit_breakpoint_ids: None,
            }).ok(),
        })
    }

    /// Send a terminated event
    pub async fn send_terminated_event(&mut self) -> Option<Event> {
        Some(Event {
            seq: self.next_seq(),
            message_type: "event".to_string(),
            event: "terminated".to_string(),
            body: serde_json::to_value(&TerminatedEventBody { restart: None }).ok(),
        })
    }

    /// Send an output event
    pub async fn send_output_event(&mut self, output: String, category: Option<String>) -> Option<Event> {
        Some(Event {
            seq: self.next_seq(),
            message_type: "event".to_string(),
            event: "output".to_string(),
            body: serde_json::to_value(&OutputEventBody {
                category,
                output,
                group: None,
                variables_reference: None,
                source: None,
                line: None,
                column: None,
                data: None,
            }).ok(),
        })
    }

    /// Get current debug state
    pub fn state(&self) -> DebugState {
        self.state
    }

    /// Get mutable reference to VM
    pub fn vm_mut(&mut self) -> Option<&mut VM> {
        self.vm.as_mut()
    }
}

impl Default for DebugSession {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert a VM value to a DAP variable
fn value_to_variable(name: &str, value: &Value, parent_ref: i64) -> Variable {
    let (value_str, var_type, variables_ref) = match value {
        Value::String(s) => (format!("\"{}\"", s), Some("string".to_string()), 0),
        Value::Bool(b) => (b.to_string(), Some("bool".to_string()), 0),
        Value::Null => ("null".to_string(), Some("null".to_string()), 0),
        Value::Int8(i) => (i.to_string(), Some("i8".to_string()), 0),
        Value::Int16(i) => (i.to_string(), Some("i16".to_string()), 0),
        Value::Int32(i) => (i.to_string(), Some("i32".to_string()), 0),
        Value::Int64(i) => (i.to_string(), Some("i64".to_string()), 0),
        Value::UInt8(i) => (i.to_string(), Some("u8".to_string()), 0),
        Value::UInt16(i) => (i.to_string(), Some("u16".to_string()), 0),
        Value::UInt32(i) => (i.to_string(), Some("u32".to_string()), 0),
        Value::UInt64(i) => (i.to_string(), Some("u64".to_string()), 0),
        Value::Float32(f) => (f.to_string(), Some("f32".to_string()), 0),
        Value::Float64(f) => (f.to_string(), Some("f64".to_string()), 0),
        Value::Array(arr) => {
            let len = arr.lock().unwrap().len();
            (format!("[{} items]", len), Some("array".to_string()), 100 + parent_ref)
        }
        Value::Instance(inst) => {
            let inst = inst.lock().unwrap();
            let class_name = &inst.class;
            (format!("<{}>", class_name), Some(class_name.clone()), 200 + parent_ref)
        }
        Value::Exception(ex) => (format!("Exception: {}", ex.message), Some("exception".to_string()), 0),
        Value::Promise(_) => ("<Promise>".to_string(), Some("Promise".to_string()), 0),
    };

    Variable {
        name: name.to_string(),
        value: value_str,
        variable_type: Some(var_type.unwrap_or("unknown".to_string())),
        variables_reference: variables_ref,
        named_variables: None,
        indexed_variables: None,
        memory_reference: None,
        presentation_hint: None,
        evaluate_name: Some(name.to_string()),
    }
}
