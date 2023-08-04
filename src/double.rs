// #[export_name = "\x01foo"]
#[no_mangle]
pub extern "C" fn foo() {}

// #[export_name = "\x01double_int"]
#[no_mangle]
pub extern "C" fn double_int(input: i32) -> i32 {
    input * 2
}

#[no_mangle]
// #[export_name = "\x01rust_function"]
pub extern "C" fn rust_function() {}
