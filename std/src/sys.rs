use sparkler::vm::Instance;
use sparkler::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

pub struct ProcessData {
    child: Option<Child>,
    stdin_handle: Option<std::process::ChildStdin>,
    stdout_handle: Option<std::process::ChildStdout>,
    stderr_handle: Option<std::process::ChildStderr>,
    exit_code: Option<i32>,
    stdout_captured: Vec<u8>,
    stderr_captured: Vec<u8>,
}

impl ProcessData {
    fn new() -> Self {
        ProcessData {
            child: None,
            stdin_handle: None,
            stdout_handle: None,
            stderr_handle: None,
            exit_code: None,
            stdout_captured: Vec::new(),
            stderr_captured: Vec::new(),
        }
    }
}

pub fn native_process_native_create(args: &mut Vec<Value>) -> Result<Value, Value> {
    let instance = if let Value::Instance(inst) = &args[0] {
        inst.clone()
    } else {
        return Err(Value::String("Process native_create requires instance".to_string()));
    };

    let mut inst = instance.lock().unwrap();
    inst.native_data = Arc::new(Mutex::new(Some(Box::new(ProcessData::new()))));

    Ok(Value::Null)
}

pub fn native_process_start(args: &mut Vec<Value>) -> Result<Value, Value> {
    let instance = if let Value::Instance(inst) = &args[0] {
        inst.clone()
    } else {
        return Err(Value::String("Process.start requires instance".to_string()));
    };

    let command_str = if let Value::String(s) = &args[1] {
        s.clone()
    } else {
        return Err(Value::String("Process.start requires a command string".to_string()));
    };

    let args_vec = if args.len() > 2 {
        if let Value::Instance(inst) = &args[2] {
            let inst_locked = inst.lock().unwrap();
            let native_data_locked = inst_locked.native_data.lock().unwrap();
            if let Some(data) = native_data_locked.as_ref() {
                if let Some(buffer) = data.downcast_ref::<Vec<String>>() {
                    buffer.clone()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let capture_stdout = if args.len() > 3 { matches!(&args[3], Value::Bool(true)) } else { false };
    let capture_stderr = if args.len() > 4 { matches!(&args[4], Value::Bool(true)) } else { false };
    let inherit_stdin = if args.len() > 5 { matches!(&args[5], Value::Bool(true)) } else { false };
    let working_dir = if args.len() > 6 {
        if let Value::String(s) = &args[6] {
            Some(s.clone())
        } else {
            None
        }
    } else {
        None
    };

    let mut cmd = Command::new(&command_str);
    cmd.args(&args_vec);

    if let Some(dir) = working_dir {
        cmd.current_dir(&dir);
    }

    if capture_stdout {
        cmd.stdout(Stdio::piped());
    } else {
        cmd.stdout(Stdio::inherit());
    }

    if capture_stderr {
        cmd.stderr(Stdio::piped());
    } else {
        cmd.stderr(Stdio::inherit());
    }

    if inherit_stdin {
        cmd.stdin(Stdio::inherit());
    } else {
        cmd.stdin(Stdio::piped());
    }

    let inst = instance.lock().unwrap();
    let mut native_data = inst.native_data.lock().unwrap();
    if let Some(data) = native_data.as_mut() {
        if let Some(proc_data) = data.downcast_mut::<ProcessData>() {
            match cmd.spawn() {
                Ok(mut child) => {
                    proc_data.stdin_handle = child.stdin.take();
                    proc_data.stdout_handle = child.stdout.take();
                    proc_data.stderr_handle = child.stderr.take();
                    proc_data.child = Some(child);
                    return Ok(Value::Null);
                }
                Err(e) => {
                    return Err(Value::String(format!("Failed to start process: {}", e)));
                }
            }
        }
    }

    Err(Value::String("Process native data not initialized".to_string()))
}

pub fn native_process_write_stdin(args: &mut Vec<Value>) -> Result<Value, Value> {
    let instance = if let Value::Instance(inst) = &args[0] {
        inst.clone()
    } else {
        return Err(Value::String("Process.write_stdin requires instance".to_string()));
    };

    let input = if let Value::String(s) = &args[1] {
        s.clone().into_bytes()
    } else {
        return Err(Value::String("Process.write_stdin requires a string argument".to_string()));
    };

    let inst = instance.lock().unwrap();
    let mut native_data = inst.native_data.lock().unwrap();
    if let Some(data) = native_data.as_mut() {
        if let Some(proc_data) = data.downcast_mut::<ProcessData>() {
            if let Some(stdin) = &mut proc_data.stdin_handle {
                match stdin.write_all(&input) {
                    Ok(_) => return Ok(Value::Null),
                    Err(e) => {
                        return Err(Value::String(format!("Failed to write to stdin: {}", e)))
                    }
                }
            } else {
                return Err(Value::String(
                    "Process stdin not available (may be inherited or closed)".to_string(),
                ));
            }
        }
    }

    Err(Value::String("Process native data not initialized".to_string()))
}

pub fn native_process_close_stdin(args: &mut Vec<Value>) -> Result<Value, Value> {
    let instance = if let Value::Instance(inst) = &args[0] {
        inst.clone()
    } else {
        return Err(Value::String("Process.close_stdin requires instance".to_string()));
    };

    let inst = instance.lock().unwrap();
    let mut native_data = inst.native_data.lock().unwrap();
    if let Some(data) = native_data.as_mut() {
        if let Some(proc_data) = data.downcast_mut::<ProcessData>() {
            proc_data.stdin_handle = None;
            return Ok(Value::Null);
        }
    }

    Err(Value::String("Process native data not initialized".to_string()))
}

pub fn native_process_read_stdout(args: &mut Vec<Value>) -> Result<Value, Value> {
    let instance = if let Value::Instance(inst) = &args[0] {
        inst.clone()
    } else {
        return Err(Value::String("Process.read_stdout requires instance".to_string()));
    };

    let inst = instance.lock().unwrap();
    let mut native_data = inst.native_data.lock().unwrap();
    if let Some(data) = native_data.as_mut() {
        if let Some(proc_data) = data.downcast_mut::<ProcessData>() {
            if let Some(stdout) = &mut proc_data.stdout_handle {
                let mut buffer = vec![0u8; 4096];
                match stdout.read(&mut buffer) {
                    Ok(n) if n > 0 => {
                        proc_data.stdout_captured.extend_from_slice(&buffer[..n]);
                        return Ok(Value::String(
                            String::from_utf8_lossy(&buffer[..n]).to_string(),
                        ));
                    }
                    Ok(_) => {
                        return Ok(Value::String(String::new()));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        return Ok(Value::String(String::new()));
                    }
                    Err(e) => {
                        return Err(Value::String(format!("Failed to read stdout: {}", e)));
                    }
                }
            } else {
                return Err(Value::String(
                    "Process stdout not available (may be inherited or closed)".to_string(),
                ));
            }
        }
    }

    Err(Value::String("Process native data not initialized".to_string()))
}

pub fn native_process_read_stderr(args: &mut Vec<Value>) -> Result<Value, Value> {
    let instance = if let Value::Instance(inst) = &args[0] {
        inst.clone()
    } else {
        return Err(Value::String("Process.read_stderr requires instance".to_string()));
    };

    let inst = instance.lock().unwrap();
    let mut native_data = inst.native_data.lock().unwrap();
    if let Some(data) = native_data.as_mut() {
        if let Some(proc_data) = data.downcast_mut::<ProcessData>() {
            if let Some(stderr) = &mut proc_data.stderr_handle {
                let mut buffer = vec![0u8; 4096];
                match stderr.read(&mut buffer) {
                    Ok(n) if n > 0 => {
                        proc_data.stderr_captured.extend_from_slice(&buffer[..n]);
                        return Ok(Value::String(
                            String::from_utf8_lossy(&buffer[..n]).to_string(),
                        ));
                    }
                    Ok(_) => {
                        return Ok(Value::String(String::new()));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        return Ok(Value::String(String::new()));
                    }
                    Err(e) => {
                        return Err(Value::String(format!("Failed to read stderr: {}", e)));
                    }
                }
            } else {
                return Err(Value::String(
                    "Process stderr not available (may be inherited or closed)".to_string(),
                ));
            }
        }
    }

    Err(Value::String("Process native data not initialized".to_string()))
}

pub fn native_process_wait(args: &mut Vec<Value>) -> Result<Value, Value> {
    let instance = if let Value::Instance(inst) = &args[0] {
        inst.clone()
    } else {
        return Err(Value::String("Process.wait requires instance".to_string()));
    };

    let inst = instance.lock().unwrap();
    let mut native_data = inst.native_data.lock().unwrap();
    if let Some(data) = native_data.as_mut() {
        if let Some(proc_data) = data.downcast_mut::<ProcessData>() {
            if let Some(child) = &mut proc_data.child {
                // Close stdin before waiting to avoid deadlock
                drop(proc_data.stdin_handle.take());

                match child.wait() {
                    Ok(status) => {
                        proc_data.exit_code = status.code();

                        // Read any remaining stdout
                        if let Some(mut stdout) = proc_data.stdout_handle.take() {
                            let mut buffer = Vec::new();
                            if stdout.read_to_end(&mut buffer).is_ok() {
                                proc_data.stdout_captured.extend_from_slice(&buffer);
                            }
                        }

                        // Read any remaining stderr
                        if let Some(mut stderr) = proc_data.stderr_handle.take() {
                            let mut buffer = Vec::new();
                            if stderr.read_to_end(&mut buffer).is_ok() {
                                proc_data.stderr_captured.extend_from_slice(&buffer);
                            }
                        }

                        return Ok(Value::Null);
                    }
                    Err(e) => {
                        return Err(Value::String(format!("Failed to wait for process: {}", e)));
                    }
                }
            } else {
                return Err(Value::String("Process not started or already completed".to_string()));
            }
        }
    }

    Err(Value::String("Process native data not initialized".to_string()))
}

pub fn native_process_exit_code(args: &mut Vec<Value>) -> Result<Value, Value> {
    let instance = if let Value::Instance(inst) = &args[0] {
        inst.clone()
    } else {
        return Err(Value::String("Process.exit_code requires instance".to_string()));
    };

    let inst = instance.lock().unwrap();
    let native_data = inst.native_data.lock().unwrap();
    if let Some(data) = native_data.as_ref() {
        if let Some(proc_data) = data.downcast_ref::<ProcessData>() {
            if let Some(code) = proc_data.exit_code {
                return Ok(Value::Int64(code as i64));
            } else {
                return Ok(Value::Null);
            }
        }
    }

    Err(Value::String("Process native data not initialized".to_string()))
}

pub fn native_process_get_stdout(args: &mut Vec<Value>) -> Result<Value, Value> {
    let instance = if let Value::Instance(inst) = &args[0] {
        inst.clone()
    } else {
        return Err(Value::String("Process.get_stdout requires instance".to_string()));
    };

    let inst = instance.lock().unwrap();
    let native_data = inst.native_data.lock().unwrap();
    if let Some(data) = native_data.as_ref() {
        if let Some(proc_data) = data.downcast_ref::<ProcessData>() {
            let output = String::from_utf8_lossy(&proc_data.stdout_captured).to_string();
            return Ok(Value::String(output));
        }
    }

    Err(Value::String("Process native data not initialized".to_string()))
}

pub fn native_process_get_stderr(args: &mut Vec<Value>) -> Result<Value, Value> {
    let instance = if let Value::Instance(inst) = &args[0] {
        inst.clone()
    } else {
        return Err(Value::String("Process.get_stderr requires instance".to_string()));
    };

    let inst = instance.lock().unwrap();
    let native_data = inst.native_data.lock().unwrap();
    if let Some(data) = native_data.as_ref() {
        if let Some(proc_data) = data.downcast_ref::<ProcessData>() {
            let output = String::from_utf8_lossy(&proc_data.stderr_captured).to_string();
            return Ok(Value::String(output));
        }
    }

    Err(Value::String("Process native data not initialized".to_string()))
}

pub fn native_sys_env(args: &mut Vec<Value>) -> Result<Value, Value> {
    if !args.is_empty() {
        if let Value::String(key) = &args[0] {
            return match std::env::var(key) {
                Ok(val) => Ok(Value::String(val)),
                Err(_) => Ok(Value::Null),
            };
        } else if let Value::Null = &args[0] {
            // Fall through to returning all env vars
        } else {
            return Err(Value::String(
                "env requires a string argument or null".to_string(),
            ));
        }
    }

    let mut env_map = HashMap::new();
    for (key, value) in std::env::vars() {
        env_map.insert(key, Value::String(value));
    }

    Ok(Value::Instance(Arc::new(Mutex::new(Instance {
        class: "Object".to_string(),
        fields: env_map,
        native_data: Arc::new(Mutex::new(None)),
    }))))
}

pub fn native_sys_set_pwd(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String("set_pwd requires a directory argument".to_string()));
    }

    let dir = if let Value::String(s) = &args[0] {
        s.clone()
    } else {
        return Err(Value::String("set_pwd requires a string argument".to_string()));
    };

    match std::env::set_current_dir(&dir) {
        Ok(()) => Ok(Value::Null),
        Err(e) => Err(Value::String(format!("Failed to change directory: {}", e))),
    }
}
