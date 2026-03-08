pub mod io;
pub mod http;
pub mod json;
pub mod reflect;
pub mod sys;
pub mod fs;
pub mod ffi;

use sparkler::{VM, Value};

pub fn register_all(vm: &mut VM) {
    // Global built-ins
    vm.register_native("print", io::native_print);
    vm.register_native("println", io::native_println);

    // IO
    vm.register_native("std::io::print", io::native_print);
    vm.register_native("std::io::println", io::native_println);

    // HTTP
    vm.register_native("std::http::get", http::native_http_get);
    vm.register_native("std::http::post", http::native_http_post);

    // JSON
    vm.register_native("std::json::stringify", json::native_json_stringify);
    vm.register_native("std::json::parse", json::native_json_parse);

    // Reflection
    vm.register_native("std::reflect::type_of", reflect::native_reflect_typeof);
    vm.register_native("std::reflect::class_name", reflect::native_reflect_class_name);
    vm.register_native("std::reflect::fields", reflect::native_reflect_fields);

    // Sys
    vm.register_native("std::sys::env", sys::native_sys_env);

    // Fallback function that throws an error
    vm.register_fallback(|_args| {
        Err(Value::String("Native method not available or disabled by runtime".to_string()))
    });
}
