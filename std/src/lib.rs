pub mod ffi;
pub mod fs;
pub mod http;
pub mod io;
pub mod json;
pub mod reflect;
pub mod sys;

use sparkler::{NativeModule, Value, VM};

pub fn register_all(vm: &mut VM) {
    vm.native("print", io::native_print)
        .description("Print without newline")
        .register(vm);
    vm.native("println", io::native_println)
        .description("Print with newline")
        .register(vm);

    NativeModule::new("std::io")
        .function("print", io::native_print)
        .function("println", io::native_println)
        .register(vm);

    NativeModule::new("std::http")
        .function("get", http::native_http_get)
        .function("post", http::native_http_post)
        .class_method(
            "HttpClient",
            "set_timeout",
            http::native_http_client_set_timeout,
        )
        .class_method(
            "HttpClient",
            "set_base_url",
            http::native_http_client_set_base_url,
        )
        .class_method(
            "HttpClient",
            "add_header",
            http::native_http_client_add_header,
        )
        .class_method("HttpClient", "get", http::native_http_client_get)
        .class_method("HttpClient", "post", http::native_http_client_post)
        .register(vm);

    NativeModule::new("std::json")
        .function("stringify", json::native_json_stringify)
        .function("parse", json::native_json_parse)
        .register(vm);

    NativeModule::new("std::reflect")
        .function("type_of", reflect::native_reflect_typeof)
        .function("class_name", reflect::native_reflect_class_name)
        .function("fields", reflect::native_reflect_fields)
        .register(vm);

    NativeModule::new("std::sys")
        .function("env", sys::native_sys_env)
        .register(vm);

    NativeModule::new("std::fs")
        .function("read", fs::native_fs_read)
        .function("read_string", fs::native_fs_read_string)
        .function("write", fs::native_fs_write)
        .function("write_string", fs::native_fs_write_string)
        .function("append", fs::native_fs_append)
        .function("append_string", fs::native_fs_append_string)
        .function("remove", fs::native_fs_remove)
        .function("remove_file", fs::native_fs_remove_file)
        .function("remove_dir", fs::native_fs_remove_dir)
        .function("remove_dir_all", fs::native_fs_remove_dir_all)
        .function("exists", fs::native_fs_exists)
        .function("is_file", fs::native_fs_is_file)
        .function("is_dir", fs::native_fs_is_dir)
        .function("create_dir", fs::native_fs_create_dir)
        .function("create_dir_all", fs::native_fs_create_dir_all)
        .function("read_dir", fs::native_fs_read_dir)
        .function("copy", fs::native_fs_copy)
        .function("rename", fs::native_fs_rename)
        .function("metadata", fs::native_fs_metadata)
        .function("canonicalize", fs::native_fs_canonicalize)
        .register(vm);

    // Fallback function that throws an error
    vm.register_fallback(|_args| {
        Err(Value::String(
            "Native method not available or disabled by runtime".to_string(),
        ))
    });
}
