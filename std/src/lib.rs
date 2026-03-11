pub mod args;
pub mod data;
pub mod ffi;
pub mod fs;
pub mod http;
pub mod io;
pub mod json;
pub mod math;
pub mod reflect;
pub mod str;
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

    NativeModule::new("std::data")
        .class_native_create("ByteBuffer", data::native_byte_buffer_native_create)
        .class_method("ByteBuffer", "constructor", data::native_byte_buffer_constructor)
        .class_method("ByteBuffer", "reserve", data::native_byte_buffer_reserve)
        .class_method("ByteBuffer", "get", data::native_byte_buffer_get)
        .class_method("ByteBuffer", "set", data::native_byte_buffer_set)
        .class_method("ByteBuffer", "length", data::native_byte_buffer_length)
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

    NativeModule::new("")
        .class_method("str", "length", str::native_str_length)
        .class_method("str", "trim", str::native_str_trim)
        .class_method("str", "split", str::native_str_split)
        .class_method("str", "to_int", str::native_str_to_int)
        .class_method("str", "to_float", str::native_str_to_float)
        .class_method("str", "contains", str::native_str_contains)
        .class_method("str", "starts_with", str::native_str_starts_with)
        .class_method("str", "ends_with", str::native_str_ends_with)
        .class_method("str", "substring", str::native_str_substring)
        .class_method("str", "to_lowercase", str::native_str_to_lowercase)
        .class_method("str", "to_uppercase", str::native_str_to_uppercase)
        .class_method("str", "replace", str::native_str_replace)
        .register(vm);

    NativeModule::new("std::sys")
        .function("env", sys::native_sys_env)
        .function("set_pwd", sys::native_sys_set_pwd)
        .class_native_create("Process", sys::native_process_native_create)
        .class_method("Process", "start", sys::native_process_start)
        .class_method("Process", "write_stdin", sys::native_process_write_stdin)
        .class_method("Process", "close_stdin", sys::native_process_close_stdin)
        .class_method("Process", "read_stdout", sys::native_process_read_stdout)
        .class_method("Process", "read_stderr", sys::native_process_read_stderr)
        .class_method("Process", "wait", sys::native_process_wait)
        .class_method("Process", "exit_code", sys::native_process_exit_code)
        .class_method("Process", "get_stdout", sys::native_process_get_stdout)
        .class_method("Process", "get_stderr", sys::native_process_get_stderr)
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

    NativeModule::new("std::args")
        .function("get", args::native_args_get)
        .register(vm);

    // Register math constants and functions
    NativeModule::new("std::math")
        .function("sin", math::native_math_sin)
        .function("cos", math::native_math_cos)
        .function("tan", math::native_math_tan)
        .function("asin", math::native_math_asin)
        .function("acos", math::native_math_acos)
        .function("atan", math::native_math_atan)
        .function("atan2", math::native_math_atan2)
        .function("sinh", math::native_math_sinh)
        .function("cosh", math::native_math_cosh)
        .function("tanh", math::native_math_tanh)
        .function("asinh", math::native_math_asinh)
        .function("acosh", math::native_math_acosh)
        .function("atanh", math::native_math_atanh)
        .function("floor", math::native_math_floor)
        .function("ceil", math::native_math_ceil)
        .function("round", math::native_math_round)
        .function("trunc", math::native_math_trunc)
        .function("fract", math::native_math_fract)
        .function("min", math::native_math_min)
        .function("max", math::native_math_max)
        .function("clamp", math::native_math_clamp)
        .function("abs", math::native_math_abs)
        .function("sign", math::native_math_sign)
        .function("sqrt", math::native_math_sqrt)
        .function("cbrt", math::native_math_cbrt)
        .function("pow", math::native_math_pow)
        .function("exp", math::native_math_exp)
        .function("ln", math::native_math_ln)
        .function("log10", math::native_math_log10)
        .function("log2", math::native_math_log2)
        .function("log", math::native_math_log)
        .function("hypot", math::native_math_hypot)
        .function("lerp", math::native_math_lerp)
        .function("step", math::native_math_step)
        .function("smoothstep", math::native_math_smoothstep)
        .function("toRadians", math::native_math_to_radians)
        .function("toDegrees", math::native_math_to_degrees)
        .register(vm);

    // Fallback function that throws an error
    vm.register_fallback(|_args| {
        Err(Value::String(
            "Native method not available or disabled by runtime".to_string(),
        ))
    });
}
