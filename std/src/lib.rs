pub mod args;
pub mod data;
pub mod ffi;
pub mod fs;
pub mod http;
pub mod io;
pub mod json;
pub mod math;
pub mod random;
pub mod reflect;
pub mod str;
pub mod sys;
pub mod test;

use sparkler::{NativeModule, NativeResult, Value, VM};

pub fn register_all(vm: &mut VM) {
    NativeModule::new("std.io")
        .function("print(str)", io::native_print)
        .function("println(str)", io::native_println)
        .register(vm);

    NativeModule::new("std.data")
        .class("ByteBuffer")
            .native_create(data::native_byte_buffer_native_create)
            .method("constructor", data::native_byte_buffer_constructor)
            .method("reserve", data::native_byte_buffer_reserve)
            .method("get", data::native_byte_buffer_get)
            .method("set", data::native_byte_buffer_set)
            .method("length", data::native_byte_buffer_length)
            .register_class()
        .register(vm);

    NativeModule::new("std.http")
        .function("get", http::native_http_get)
        .function("post", http::native_http_post)
        .class("HttpClient")
            .native_create(http::native_http_client_native_create)
            .method("constructor()", http::native_http_client_constructor)
            .method("setTimeout(int)", http::native_http_client_set_timeout)
            .method("setBaseUrl(str)", http::native_http_client_set_base_url)
            .method("setRedirectPolicy(std.http.RedirectPolicy)", http::native_http_client_set_redirect_policy)
            .method("setMaxRedirects(int)", http::native_http_client_set_max_redirects)
            .method("setProxy(str,int)", http::native_http_client_set_proxy)
            .method("setVerifySsl(bool)", http::native_http_client_set_verify_ssl)
            .method("addHeader(str,str)", http::native_http_client_add_header)
            .method("get(str)", http::native_http_client_get)
            .method("post(str,str)", http::native_http_client_post)
            .register_class()
        .register(vm);

    NativeModule::new("std.json")
        .function("stringify", json::native_json_stringify)
        .function("parse", json::native_json_parse)
        .register(vm);

    NativeModule::new("std.reflect")
        .function("type_of", reflect::native_reflect_typeof)
        .function("class_name", reflect::native_reflect_class_name)
        .function("fields", reflect::native_reflect_fields)
        .register(vm);

    NativeModule::new("")
        .class("str")
            .method("length", str::native_str_length)
            .method("trim", str::native_str_trim)
            .method("split", str::native_str_split)
            .method("toInt", str::native_str_to_int)
            .method("toFloat", str::native_str_to_float)
            .method("contains", str::native_str_contains)
            .method("startsWith", str::native_str_starts_with)
            .method("endsWith", str::native_str_ends_with)
            .method("substring", str::native_str_substring)
            .method("toLower", str::native_str_to_lowercase)
            .method("toUpper", str::native_str_to_uppercase)
            .method("replace", str::native_str_replace)
            .register_class()
        .register(vm);

    // Register global str() function separately
    NativeModule::new("")
        .function("str(unknown)", str::native_str)
        .function("int(unknown)", str::native_int)
        .function("float(unknown)", str::native_float)
        .function("bool(unknown)", str::native_bool)
        .function("int8(unknown)", str::native_int8)
        .function("uint8(unknown)", str::native_uint8)
        .function("int16(unknown)", str::native_int16)
        .function("uint16(unknown)", str::native_uint16)
        .function("int32(unknown)", str::native_int32)
        .function("uint32(unknown)", str::native_uint32)
        .function("int64(unknown)", str::native_int64)
        .function("uint64(unknown)", str::native_uint64)
        .function("float32(unknown)", str::native_float32)
        .function("float64(unknown)", str::native_float64)
        .register(vm);

    NativeModule::new("std.sys")
        .function("env", sys::native_sys_env)
        .function("set_pwd", sys::native_sys_set_pwd)
        .class("Process")
            .native_create(sys::native_process_native_create)
            .native_destroy(sys::native_process_native_destroy)
            .method("start", sys::native_process_start)
            .method("write_stdin", sys::native_process_write_stdin)
            .method("close_stdin", sys::native_process_close_stdin)
            .method("read_stdout", sys::native_process_read_stdout)
            .method("read_stderr", sys::native_process_read_stderr)
            .method("wait", sys::native_process_wait)
            .method("exit_code", sys::native_process_exit_code)
            .method("get_stdout", sys::native_process_get_stdout)
            .method("get_stderr", sys::native_process_get_stderr)
            .register_class()
        .register(vm);

    NativeModule::new("std.fs")
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

    NativeModule::new("std.args")
        .function("get", args::native_args_get)
        .register(vm);

    NativeModule::new("std.math")
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
        .function("check_overflow", math::native_math_check_overflow)
        .function("check_div_zero", math::native_math_check_div_zero)
        .register(vm);

    NativeModule::new("std.random")
        .function("nextBool()", random::native_random_next_bool)
        .function("nextInt()", random::native_random_next_int)
        .function("nextIntRange(int,int)", random::native_random_next_int_range)
        .function("nextFloat()", random::native_random_next_float)
        .function("nextFloatRange(float,float)", random::native_random_next_float_range)
        .register(vm);

    NativeModule::new("std.test")
        .function("addFailure", test::native_fail)
        .function("recordPass", test::native_record_pass)
        .function("setCurrentTest", test::native_set_current_test)
        .function("assertSame", test::native_assert_same)
        .register(vm);

    vm.register_fallback(|_args| {
        NativeResult::Ready(Value::String(
            "Native method not available or disabled by runtime".to_string(),
        ))
    });
}
