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
pub mod time;

use sparkler::{NativeFallbackFn, NativeModule, NativeResult, Value, VM};

pub fn register_all(vm: &mut VM) {
    NativeModule::new("std.io")
        .function("print(str)", io::native_print)
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
        .function("stringify(*)", json::native_json_stringify)
        .function("parse(str)", json::native_json_parse)
        .register(vm);

    NativeModule::new("std.reflect")
        .function("type_of(*)", reflect::native_reflect_typeof)
        .function("class_name(*)", reflect::native_reflect_class_name)
        .function("fields(*)", reflect::native_reflect_fields)
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
        .function("str(*)", str::native_str)
        .function("int(*)", str::native_int)
        .function("float(*)", str::native_float)
        .function("bool(*)", str::native_bool)
        .function("int8(*)", str::native_int8)
        .function("uint8(*)", str::native_uint8)
        .function("int16(*)", str::native_int16)
        .function("uint16(*)", str::native_uint16)
        .function("int32(*)", str::native_int32)
        .function("uint32(*)", str::native_uint32)
        .function("int64(*)", str::native_int64)
        .function("uint64(*)", str::native_uint64)
        .function("float32(*)", str::native_float32)
        .function("float64(*)", str::native_float64)
        .register(vm);

    NativeModule::new("std.sys")
        .function("env(str)", sys::native_sys_env)
        .function("setPwd(str)", sys::native_sys_set_pwd)
        .class("Process")
            .native_create(sys::native_process_native_create)
            .native_destroy(sys::native_process_native_destroy)
            .method("start(str,str[],bool,bool,bool,str?)", sys::native_process_start)
            .method("writeStdin(str)", sys::native_process_write_stdin)
            .method("closeStdin()", sys::native_process_close_stdin)
            .method("readStdout()", sys::native_process_read_stdout)
            .method("readStderr()", sys::native_process_read_stderr)
            .method("wait()", sys::native_process_wait)
            .method("exitCode()", sys::native_process_exit_code)
            .method("getStdout()", sys::native_process_get_stdout)
            .method("getStderr()", sys::native_process_get_stderr)
            .register_class()
        .register(vm);

    NativeModule::new("std.fs")
        .function("read(str)", fs::native_fs_read)
        .function("readString(str)", fs::native_fs_read_string)
        .function("write(str,Array)", fs::native_fs_write)
        .function("writeString(str,str)", fs::native_fs_write_string)
        .function("append(str,Array)", fs::native_fs_append)
        .function("appendString(str,str)", fs::native_fs_append_string)
        .function("remove(str)", fs::native_fs_remove)
        .function("removeFile(str)", fs::native_fs_remove_file)
        .function("removeDir(str)", fs::native_fs_remove_dir)
        .function("removeDirAll(str)", fs::native_fs_remove_dir_all)
        .function("exists(str)", fs::native_fs_exists)
        .function("isFile(str)", fs::native_fs_is_file)
        .function("isDir(str)", fs::native_fs_is_dir)
        .function("createDir(str)", fs::native_fs_create_dir)
        .function("createDirAll(str)", fs::native_fs_create_dir_all)
        .function("readDir(str)", fs::native_fs_read_dir)
        .function("copy(str,str)", fs::native_fs_copy)
        .function("rename(str,str)", fs::native_fs_rename)
        .function("metadata(str)", fs::native_fs_metadata)
        .function("canonicalize(str)", fs::native_fs_canonicalize)
        .register(vm);

    NativeModule::new("std.args")
        .function("get()", args::native_args_get)
        .register(vm);

    NativeModule::new("std.math")
        .function("sin(float)", math::native_math_sin)
        .function("cos(float)", math::native_math_cos)
        .function("tan(float)", math::native_math_tan)
        .function("asin(float)", math::native_math_asin)
        .function("acos(float)", math::native_math_acos)
        .function("atan(float)", math::native_math_atan)
        .function("atan2(float,float)", math::native_math_atan2)
        .function("sinh(float)", math::native_math_sinh)
        .function("cosh(float)", math::native_math_cosh)
        .function("tanh(float)", math::native_math_tanh)
        .function("asinh(float)", math::native_math_asinh)
        .function("acosh(float)", math::native_math_acosh)
        .function("atanh(float)", math::native_math_atanh)
        .function("floor(float)", math::native_math_floor)
        .function("ceil(float)", math::native_math_ceil)
        .function("round(float)", math::native_math_round)
        .function("trunc(float)", math::native_math_trunc)
        .function("fract(float)", math::native_math_fract)
        .function("min(float,float)", math::native_math_min)
        .function("max(float,float)", math::native_math_max)
        .function("clamp(float,float,float)", math::native_math_clamp)
        .function("abs(float)", math::native_math_abs)
        .function("sign(float)", math::native_math_sign)
        .function("sqrt(float)", math::native_math_sqrt)
        .function("cbrt(float)", math::native_math_cbrt)
        .function("pow(float,float)", math::native_math_pow)
        .function("exp(float)", math::native_math_exp)
        .function("ln(float)", math::native_math_ln)
        .function("log10(float)", math::native_math_log10)
        .function("log2(float)", math::native_math_log2)
        .function("log(float,float)", math::native_math_log)
        .function("hypot(float,float)", math::native_math_hypot)
        .function("lerp(float,float,float)", math::native_math_lerp)
        .function("step(float,float)", math::native_math_step)
        .function("smoothstep(float,float,float)", math::native_math_smoothstep)
        .function("toRadians(float)", math::native_math_to_radians)
        .function("toDegrees(float)", math::native_math_to_degrees)
        .function("check_overflow(int)", math::native_math_check_overflow)
        .function("check_div_zero(float)", math::native_math_check_div_zero)
        .register(vm);

    NativeModule::new("std.random")
        .function("nextBool()", random::native_random_next_bool)
        .function("nextInt()", random::native_random_next_int)
        .function("nextIntRange(int,int)", random::native_random_next_int_range)
        .function("nextFloat()", random::native_random_next_float)
        .function("nextFloatRange(float,float)", random::native_random_next_float_range)
        .register(vm);

    NativeModule::new("std.time")
        .function("CurrentTime()", time::native_time_current_time)
        .function("CurrentHour()", time::native_time_current_hour)
        .function("CurrentMin()", time::native_time_current_min)
        .function("CurrentSec()", time::native_time_current_sec)
        .register(vm);

    NativeModule::new("std.test")
        .function("addFailure", test::native_fail)
        .function("recordPass", test::native_record_pass)
        .function("setCurrentTest", test::native_set_current_test)
        .function("assertSame", test::native_assert_same)
        .register(vm);

    vm.register_fallback(|name, _args| {
        NativeResult::Ready(Value::String(
            format!("Native method not available or disabled by runtime: {}", name),
        ))
    });
}
