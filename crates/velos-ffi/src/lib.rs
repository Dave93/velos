use std::ffi::{c_char, c_int, CStr, CString};

extern "C" {
    fn velos_ping() -> *const c_char;
    fn velos_daemon_init(socket_path: *const c_char, state_dir: *const c_char) -> c_int;
    fn velos_daemon_run() -> c_int;
    fn velos_daemon_shutdown() -> c_int;
}

pub fn ping() -> String {
    unsafe {
        let ptr = velos_ping();
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

pub fn daemon_init(socket_path: Option<&str>, state_dir: Option<&str>) -> Result<(), i32> {
    let sock_c = socket_path.map(|s| CString::new(s).unwrap());
    let dir_c = state_dir.map(|s| CString::new(s).unwrap());

    let sock_ptr = sock_c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
    let dir_ptr = dir_c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());

    let ret = unsafe { velos_daemon_init(sock_ptr, dir_ptr) };
    if ret == 0 {
        Ok(())
    } else {
        Err(ret)
    }
}

pub fn daemon_run() -> Result<(), i32> {
    let ret = unsafe { velos_daemon_run() };
    if ret == 0 {
        Ok(())
    } else {
        Err(ret)
    }
}

pub fn daemon_shutdown() -> Result<(), i32> {
    let ret = unsafe { velos_daemon_shutdown() };
    if ret == 0 {
        Ok(())
    } else {
        Err(ret)
    }
}
