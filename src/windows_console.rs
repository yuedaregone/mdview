#[cfg(all(target_os = "windows", not(debug_assertions)))]
use std::ffi::c_void;

#[cfg(all(target_os = "windows", not(debug_assertions)))]
pub fn attach_parent_console() -> bool {
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;

    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(dw_process_id: u32) -> i32;
    }

    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) != 0 }
}

#[cfg(all(target_os = "windows", not(debug_assertions)))]
pub fn has_standard_output_handles() -> bool {
    const STD_OUTPUT_HANDLE: u32 = -10i32 as u32;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(n_std_handle: u32) -> *mut c_void;
    }

    let stdout = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    let stderr = unsafe { GetStdHandle(STD_ERROR_HANDLE) };
    is_valid_handle(stdout) && is_valid_handle(stderr)
}

#[cfg(all(target_os = "windows", not(debug_assertions)))]
pub fn bind_standard_streams() {
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;

    bind_input_stream();
    bind_output_stream("CONOUT$", STD_OUTPUT_HANDLE);
    bind_output_stream("CONOUT$", STD_ERROR_HANDLE);
}

#[cfg(all(target_os = "windows", not(debug_assertions)))]
fn bind_input_stream() {
    use std::fs::OpenOptions;
    use std::os::windows::io::IntoRawHandle;

    const STD_INPUT_HANDLE: u32 = -10i32 as u32;

    #[link(name = "kernel32")]
    extern "system" {
        fn SetStdHandle(n_std_handle: u32, handle: *mut c_void) -> i32;
    }

    if let Ok(file) = OpenOptions::new().read(true).open("CONIN$") {
        let handle = file.into_raw_handle();
        unsafe {
            let _ = SetStdHandle(STD_INPUT_HANDLE, handle as *mut c_void);
        }
    }
}

#[cfg(all(target_os = "windows", not(debug_assertions)))]
fn bind_output_stream(device: &str, std_handle: u32) {
    use std::fs::OpenOptions;
    use std::os::windows::io::IntoRawHandle;

    #[link(name = "kernel32")]
    extern "system" {
        fn SetStdHandle(n_std_handle: u32, handle: *mut c_void) -> i32;
    }

    if let Ok(file) = OpenOptions::new().write(true).open(device) {
        let handle = file.into_raw_handle();
        unsafe {
            let _ = SetStdHandle(std_handle, handle as *mut c_void);
        }
    }
}

#[cfg(all(target_os = "windows", not(debug_assertions)))]
fn is_valid_handle(handle: *mut c_void) -> bool {
    !handle.is_null() && handle as isize != -1
}
