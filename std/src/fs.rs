use sparkler::{vm::Instance, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Read entire file content as bytes (returns Int8 array instance)
pub fn native_fs_read(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "fs::read requires a path argument".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::read requires a string path argument".to_string(),
            ))
        }
    };

    match fs::read(path) {
        Ok(bytes) => {
            // Return as Int8 array instance
            let mut fields = HashMap::new();
            for (i, byte) in bytes.iter().enumerate() {
                fields.insert(i.to_string(), Value::Int8(*byte as i8));
            }
            fields.insert("length".to_string(), Value::Int64(bytes.len() as i64));

            Ok(Value::Instance(Arc::new(Mutex::new(Instance {
                class: "Array".to_string(),
                fields,
                native_data: Arc::new(Mutex::new(None)),
            }))))
        }
        Err(e) => Err(Value::String(format!("Failed to read file: {}", e))),
    }
}

/// Read entire file content as string
pub fn native_fs_read_string(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "fs::read_string requires a path argument".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::read_string requires a string path argument".to_string(),
            ))
        }
    };

    match fs::read_to_string(path) {
        Ok(content) => Ok(Value::String(content)),
        Err(e) => Err(Value::String(format!("Failed to read file: {}", e))),
    }
}

/// Write bytes to a file (from Int8 array instance)
pub fn native_fs_write(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.len() < 2 {
        return Err(Value::String(
            "fs::write requires path and data arguments".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::write requires a string path argument".to_string(),
            ))
        }
    };

    let bytes = extract_bytes_from_value(&args[1])?;

    match fs::write(path, bytes) {
        Ok(_) => Ok(Value::Null),
        Err(e) => Err(Value::String(format!("Failed to write file: {}", e))),
    }
}

/// Write string to a file
pub fn native_fs_write_string(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.len() < 2 {
        return Err(Value::String(
            "fs::write_string requires path and data arguments".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::write_string requires a string path argument".to_string(),
            ))
        }
    };

    let content = match &args[1] {
        Value::String(s) => s,
        _ => {
            return Err(Value::String(
                "fs::write_string requires a string data argument".to_string(),
            ))
        }
    };

    match fs::write(path, content) {
        Ok(_) => Ok(Value::Null),
        Err(e) => Err(Value::String(format!("Failed to write file: {}", e))),
    }
}

/// Append bytes to a file
pub fn native_fs_append(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.len() < 2 {
        return Err(Value::String(
            "fs::append requires path and data arguments".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::append requires a string path argument".to_string(),
            ))
        }
    };

    let bytes = extract_bytes_from_value(&args[1])?;

    match fs::OpenOptions::new().create(true).append(true).open(path) {
        Ok(mut file) => {
            use std::io::Write;
            match file.write_all(&bytes) {
                Ok(_) => Ok(Value::Null),
                Err(e) => Err(Value::String(format!("Failed to append to file: {}", e))),
            }
        }
        Err(e) => Err(Value::String(format!(
            "Failed to open file for appending: {}",
            e
        ))),
    }
}

/// Append string to a file
pub fn native_fs_append_string(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.len() < 2 {
        return Err(Value::String(
            "fs::append_string requires path and data arguments".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::append_string requires a string path argument".to_string(),
            ))
        }
    };

    let content = match &args[1] {
        Value::String(s) => s,
        _ => {
            return Err(Value::String(
                "fs::append_string requires a string data argument".to_string(),
            ))
        }
    };

    match fs::OpenOptions::new().create(true).append(true).open(path) {
        Ok(mut file) => {
            use std::io::Write;
            match file.write_all(content.as_bytes()) {
                Ok(_) => Ok(Value::Null),
                Err(e) => Err(Value::String(format!("Failed to append to file: {}", e))),
            }
        }
        Err(e) => Err(Value::String(format!(
            "Failed to open file for appending: {}",
            e
        ))),
    }
}

/// Remove a file or directory
pub fn native_fs_remove(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "fs::remove requires a path argument".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::remove requires a string path argument".to_string(),
            ))
        }
    };

    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return Err(Value::String(format!("Failed to access path: {}", e))),
    };

    if metadata.is_file() {
        match fs::remove_file(path) {
            Ok(_) => Ok(Value::Null),
            Err(e) => Err(Value::String(format!("Failed to remove file: {}", e))),
        }
    } else {
        match fs::remove_dir(path) {
            Ok(_) => Ok(Value::Null),
            Err(e) => Err(Value::String(format!("Failed to remove directory: {}", e))),
        }
    }
}

/// Remove a file
pub fn native_fs_remove_file(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "fs::remove_file requires a path argument".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::remove_file requires a string path argument".to_string(),
            ))
        }
    };

    match fs::remove_file(path) {
        Ok(_) => Ok(Value::Null),
        Err(e) => Err(Value::String(format!("Failed to remove file: {}", e))),
    }
}

/// Remove an empty directory
pub fn native_fs_remove_dir(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "fs::remove_dir requires a path argument".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::remove_dir requires a string path argument".to_string(),
            ))
        }
    };

    match fs::remove_dir(path) {
        Ok(_) => Ok(Value::Null),
        Err(e) => Err(Value::String(format!("Failed to remove directory: {}", e))),
    }
}

/// Remove a directory and all its contents
pub fn native_fs_remove_dir_all(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "fs::remove_dir_all requires a path argument".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::remove_dir_all requires a string path argument".to_string(),
            ))
        }
    };

    match fs::remove_dir_all(path) {
        Ok(_) => Ok(Value::Null),
        Err(e) => Err(Value::String(format!("Failed to remove directory: {}", e))),
    }
}

/// Check if a path exists
pub fn native_fs_exists(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "fs::exists requires a path argument".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::exists requires a string path argument".to_string(),
            ))
        }
    };

    Ok(Value::Bool(Path::new(path).exists()))
}

/// Check if a path is a file
pub fn native_fs_is_file(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "fs::is_file requires a path argument".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::is_file requires a string path argument".to_string(),
            ))
        }
    };

    Ok(Value::Bool(Path::new(path).is_file()))
}

/// Check if a path is a directory
pub fn native_fs_is_dir(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "fs::is_dir requires a path argument".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::is_dir requires a string path argument".to_string(),
            ))
        }
    };

    Ok(Value::Bool(Path::new(path).is_dir()))
}

/// Create a new directory
pub fn native_fs_create_dir(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "fs::create_dir requires a path argument".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::create_dir requires a string path argument".to_string(),
            ))
        }
    };

    match fs::create_dir(path) {
        Ok(_) => Ok(Value::Null),
        Err(e) => Err(Value::String(format!("Failed to create directory: {}", e))),
    }
}

/// Create a directory and all parent directories
pub fn native_fs_create_dir_all(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "fs::create_dir_all requires a path argument".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::create_dir_all requires a string path argument".to_string(),
            ))
        }
    };

    match fs::create_dir_all(path) {
        Ok(_) => Ok(Value::Null),
        Err(e) => Err(Value::String(format!("Failed to create directory: {}", e))),
    }
}

/// Read directory contents
pub fn native_fs_read_dir(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "fs::read_dir requires a path argument".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::read_dir requires a string path argument".to_string(),
            ))
        }
    };

    match fs::read_dir(path) {
        Ok(entries) => {
            let mut items = Vec::new();
            for entry in entries {
                match entry {
                    Ok(e) => {
                        items.push(Value::String(e.path().to_string_lossy().to_string()));
                    }
                    Err(e) => {
                        return Err(Value::String(format!(
                            "Failed to read directory entry: {}",
                            e
                        )));
                    }
                }
            }

            // Return as Array instance
            let mut fields = HashMap::new();
            for (i, item) in items.iter().enumerate() {
                fields.insert(i.to_string(), item.clone());
            }
            fields.insert("length".to_string(), Value::Int64(items.len() as i64));

            Ok(Value::Instance(Arc::new(Mutex::new(Instance {
                class: "Array".to_string(),
                fields,
                native_data: Arc::new(Mutex::new(None)),
            }))))
        }
        Err(e) => Err(Value::String(format!("Failed to read directory: {}", e))),
    }
}

/// Copy a file
pub fn native_fs_copy(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.len() < 2 {
        return Err(Value::String(
            "fs::copy requires source and destination arguments".to_string(),
        ));
    }

    let from = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::copy requires a string source path argument".to_string(),
            ))
        }
    };

    let to = match &args[1] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::copy requires a string destination path argument".to_string(),
            ))
        }
    };

    match fs::copy(from, to) {
        Ok(_) => Ok(Value::Null),
        Err(e) => Err(Value::String(format!("Failed to copy file: {}", e))),
    }
}

/// Rename/move a file or directory
pub fn native_fs_rename(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.len() < 2 {
        return Err(Value::String(
            "fs::rename requires source and destination arguments".to_string(),
        ));
    }

    let from = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::rename requires a string source path argument".to_string(),
            ))
        }
    };

    let to = match &args[1] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::rename requires a string destination path argument".to_string(),
            ))
        }
    };

    match fs::rename(from, to) {
        Ok(_) => Ok(Value::Null),
        Err(e) => Err(Value::String(format!("Failed to rename: {}", e))),
    }
}

/// Get file/directory metadata
pub fn native_fs_metadata(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "fs::metadata requires a path argument".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::metadata requires a string path argument".to_string(),
            ))
        }
    };

    match fs::metadata(path) {
        Ok(metadata) => {
            let mut fields = HashMap::new();
            fields.insert("is_file".to_string(), Value::Bool(metadata.is_file()));
            fields.insert("is_dir".to_string(), Value::Bool(metadata.is_dir()));
            fields.insert("size".to_string(), Value::Int64(metadata.len() as i64));

            // Try to get modified time
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                    fields.insert(
                        "modified".to_string(),
                        Value::Int64(duration.as_secs() as i64),
                    );
                }
            }

            // Try to get accessed time
            if let Ok(accessed) = metadata.accessed() {
                if let Ok(duration) = accessed.duration_since(std::time::UNIX_EPOCH) {
                    fields.insert(
                        "accessed".to_string(),
                        Value::Int64(duration.as_secs() as i64),
                    );
                }
            }

            // Try to get created time
            if let Ok(created) = metadata.created() {
                if let Ok(duration) = created.duration_since(std::time::UNIX_EPOCH) {
                    fields.insert(
                        "created".to_string(),
                        Value::Int64(duration.as_secs() as i64),
                    );
                }
            }

            Ok(Value::Instance(Arc::new(Mutex::new(Instance {
                class: "FsMetadata".to_string(),
                fields,
                native_data: Arc::new(Mutex::new(None)),
            }))))
        }
        Err(e) => Err(Value::String(format!("Failed to get metadata: {}", e))),
    }
}

/// Get the canonical, absolute path
pub fn native_fs_canonicalize(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "fs::canonicalize requires a path argument".to_string(),
        ));
    }

    let path = match &args[0] {
        Value::String(p) => p,
        _ => {
            return Err(Value::String(
                "fs::canonicalize requires a string path argument".to_string(),
            ))
        }
    };

    match fs::canonicalize(path) {
        Ok(canonical) => Ok(Value::String(canonical.to_string_lossy().to_string())),
        Err(e) => Err(Value::String(format!("Failed to canonicalize path: {}", e))),
    }
}

/// Helper function to extract bytes from a Value (supports Int8 array instances or strings)
fn extract_bytes_from_value(value: &Value) -> Result<Vec<u8>, Value> {
    match value {
        Value::String(s) => Ok(s.as_bytes().to_vec()),
        Value::Instance(instance) => {
            let inst = instance.lock().unwrap();
            let mut bytes = Vec::new();

            // Try to extract from array-like instance
            let mut i = 0;
            while let Some(val) = inst.fields.get(&i.to_string()) {
                if let Some(byte) = val.to_i64() {
                    bytes.push(byte as u8);
                }
                i += 1;
            }

            Ok(bytes)
        }
        _ => Err(Value::String(
            "Data must be a string or byte array".to_string(),
        )),
    }
}
