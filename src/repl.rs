use bengal_compiler::Compiler;
use sparkler::Executor;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

/// REPL state that persists across evaluations
pub struct ReplState {
    /// Accumulated source code from all entered statements
    source_history: String,
    /// Executor with registered native functions
    executor: Executor,
}

impl ReplState {
    pub fn new() -> Self {
        let mut executor = Executor::new();
        bengal_std::register_all(&mut executor.vm);
        
        Self {
            source_history: String::new(),
            executor,
        }
    }

    /// Evaluate a single line or block of input
    pub async fn evaluate(&mut self, input: &str) -> Result<Option<String>, String> {
        let trimmed = input.trim();
        let is_expr = self.is_expression(trimmed);
        
        // For expressions, we need to include history to have access to previous variables
        // For statements, we also include history for the same reason
        // But we need to handle the return value differently
        let test_source = if self.source_history.is_empty() {
            if is_expr {
                format!("return {}", trimmed)
            } else {
                trimmed.to_string()
            }
        } else {
            if is_expr {
                format!("{}\nreturn {}", self.source_history, trimmed)
            } else {
                format!("{}\n{}", self.source_history, trimmed)
            }
        };

        match self.compile_and_run(&test_source, is_expr).await {
            Ok(result) => {
                // Success - commit the input
                if !self.source_history.is_empty() {
                    self.source_history.push('\n');
                }
                self.source_history.push_str(input);
                Ok(result)
            }
            Err(e) => {
                // Check if it might be an incomplete statement
                if self.is_incomplete_statement(input) {
                    return Err(format!("incomplete: {}", e));
                }
                Err(format!("error: {}", e))
            }
        }
    }

    /// Compile and run source code, returning the last expression result if any
    async fn compile_and_run(&mut self, source: &str, is_expr: bool) -> Result<Option<String>, String> {
        let mut compiler = Compiler::new(source);
        let bytecode = match compiler.compile() {
            Ok(bc) => bc,
            Err(e) => return Err(e),
        };

        // Run the bytecode
        let result = self.executor.run_to_completion(bytecode, Some("<repl>")).await;

        match result {
            Ok(val) => {
                // For expressions, always show the result
                // For statements, never show the result (return None)
                if is_expr {
                    Ok(val.map(|v| self.format_value(&v)))
                } else {
                    // Statements don't display results
                    Ok(None)
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Check if input looks like a pure expression (vs a statement)
    fn is_expression(&self, input: &str) -> bool {
        let trimmed = input.trim();
        
        // Statements typically start with these keywords
        let statement_keywords = [
            "let ", "fn ", "class ", "interface ", "enum ", 
            "type ", "import ", "module ", "return ",
            "if ", "while ", "for ", "break", "continue",
            "try ", "catch ", "throw ",
        ];
        
        for keyword in &statement_keywords {
            if trimmed.starts_with(keyword) {
                return false;
            }
        }
        
        // If it doesn't start with a statement keyword, treat it as an expression
        true
    }

    /// Check if input looks like an incomplete statement
    fn is_incomplete_statement(&self, input: &str) -> bool {
        let trimmed = input.trim();
        
        // Check for unclosed braces
        let brace_count = input.chars().filter(|&c| c == '{').count() 
            - input.chars().filter(|&c| c == '}').count();
        if brace_count > 0 {
            return true;
        }

        // Check for unclosed parentheses
        let paren_count = input.chars().filter(|&c| c == '(').count() 
            - input.chars().filter(|&c| c == ')').count();
        if paren_count > 0 {
            return true;
        }

        // Check for unclosed brackets
        let bracket_count = input.chars().filter(|&c| c == '[').count() 
            - input.chars().filter(|&c| c == ']').count();
        if bracket_count > 0 {
            return true;
        }

        // Check for unclosed strings
        let quote_count = input.chars().filter(|&c| c == '"').count();
        if quote_count % 2 != 0 {
            return true;
        }

        // Check if line ends with operators or keywords that suggest continuation
        if trimmed.ends_with('=') 
            || trimmed.ends_with('+') 
            || trimmed.ends_with('-')
            || trimmed.ends_with('*')
            || trimmed.ends_with('/')
            || trimmed.ends_with("let ")
            || trimmed.ends_with("fn ")
            || trimmed.ends_with("if ")
            || trimmed.ends_with("while ")
            || trimmed.ends_with("for ")
            || trimmed.ends_with("return")
            || trimmed.ends_with(',')
        {
            return true;
        }

        false
    }

    /// Format a Value for display in the REPL
    fn format_value(&self, value: &sparkler::vm::Value) -> String {
        match value {
            sparkler::vm::Value::Int8(n) => n.to_string(),
            sparkler::vm::Value::Int16(n) => n.to_string(),
            sparkler::vm::Value::Int32(n) => n.to_string(),
            sparkler::vm::Value::Int64(n) => n.to_string(),
            sparkler::vm::Value::UInt8(n) => n.to_string(),
            sparkler::vm::Value::UInt16(n) => n.to_string(),
            sparkler::vm::Value::UInt32(n) => n.to_string(),
            sparkler::vm::Value::UInt64(n) => n.to_string(),
            sparkler::vm::Value::Float32(f) => f.to_string(),
            sparkler::vm::Value::Float64(f) => f.to_string(),
            sparkler::vm::Value::Bool(b) => b.to_string(),
            sparkler::vm::Value::String(s) => format!("\"{}\"", s),
            sparkler::vm::Value::Null => "()".to_string(),
            sparkler::vm::Value::Array(arr) => {
                let arr_lock = arr.lock().unwrap();
                let elements: Vec<String> = arr_lock.iter().map(|v| self.format_value(v)).collect();
                format!("[{}]", elements.join(", "))
            }
            sparkler::vm::Value::Instance(instance) => {
                let inst_lock = instance.lock().unwrap();
                format!("{} {{ .. }}", inst_lock.class)
            }
            sparkler::vm::Value::Promise(_) => "<Promise>".to_string(),
            sparkler::vm::Value::Exception(ex) => {
                format!("Exception: {}", ex.message)
            }
        }
    }

    /// Clear the REPL state
    pub fn clear(&mut self) {
        self.source_history.clear();
        // Reinitialize executor to clear any variable bindings
        let mut executor = Executor::new();
        bengal_std::register_all(&mut executor.vm);
        self.executor = executor;
    }
}

impl Default for ReplState {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the REPL interactive loop
pub async fn run_repl() -> Result<(), Box<dyn std::error::Error>> {
    let mut rl = DefaultEditor::new()?;
    let mut state = ReplState::new();

    println!("Bengal REPL v0.1.0");
    println!("Type 'exit' or Ctrl+D to quit, 'clear' to reset state");
    println!();

    loop {
        let readline = rl.readline(">>> ");
        
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.clone())?;
                
                let trimmed = line.trim();
                
                // Handle REPL commands
                if trimmed == "exit" || trimmed == "quit" {
                    break;
                }
                
                if trimmed == "clear" {
                    state.clear();
                    println!("REPL state cleared.");
                    continue;
                }

                if trimmed.is_empty() {
                    continue;
                }

                // Evaluate the input
                match state.evaluate(trimmed).await {
                    Ok(Some(result)) => {
                        println!("{}", result);
                    }
                    Ok(None) => {
                        // Statement executed successfully but no result to display
                    }
                    Err(e) => {
                        // Check if it's an incomplete statement error
                        if e.starts_with("incomplete: ") {
                            // Continue reading multi-line input
                            let mut multi_line = line.clone();
                            loop {
                                match rl.readline("... ") {
                                    Ok(next_line) => {
                                        rl.add_history_entry(next_line.clone())?;
                                        multi_line.push('\n');
                                        multi_line.push_str(&next_line);
                                        
                                        // Try to evaluate the complete input
                                        match state.evaluate(&multi_line).await {
                                            Ok(Some(result)) => {
                                                println!("{}", result);
                                                break;
                                            }
                                            Ok(None) => {
                                                break;
                                            }
                                            Err(e) => {
                                                if e.starts_with("incomplete: ") {
                                                    continue;
                                                } else {
                                                    println!("{}", e);
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    Err(ReadlineError::Interrupted) => {
                                        println!("Interrupted.");
                                        break;
                                    }
                                    Err(ReadlineError::Eof) => {
                                        break;
                                    }
                                    Err(err) => {
                                        println!("Error: {:?}", err);
                                        break;
                                    }
                                }
                            }
                        } else {
                            println!("{}", e);
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("Interrupted.");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("Exiting...");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}
