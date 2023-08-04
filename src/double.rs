#[export_name = "\x01foo"]
pub extern fn foo() {
}

// #[no_mangle]
#[export_name = "\x01double_int"]
pub extern "C" fn double_int(input: i32) -> i32 {
    input * 2
}

// #[no_mangle]
#[export_name = "\x01rust_function"]
pub extern "C" fn rust_function() {
}
