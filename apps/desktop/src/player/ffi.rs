//! The slice of libmpv's C API (client.h and render.h, API 2.x) the embedded player uses.

#![allow(non_camel_case_types, dead_code)]

use std::ffi::{c_char, c_double, c_int, c_void};

#[repr(C)]
pub struct mpv_handle {
    _private: [u8; 0],
}

#[repr(C)]
pub struct mpv_render_context {
    _private: [u8; 0],
}

pub type mpv_format = c_int;
pub const MPV_FORMAT_NONE: mpv_format = 0;
pub const MPV_FORMAT_STRING: mpv_format = 1;
pub const MPV_FORMAT_FLAG: mpv_format = 3;
pub const MPV_FORMAT_INT64: mpv_format = 4;
pub const MPV_FORMAT_DOUBLE: mpv_format = 5;
pub const MPV_FORMAT_NODE: mpv_format = 6;
pub const MPV_FORMAT_NODE_ARRAY: mpv_format = 7;
pub const MPV_FORMAT_NODE_MAP: mpv_format = 8;
pub const MPV_FORMAT_BYTE_ARRAY: mpv_format = 9;

pub type mpv_event_id = c_int;
pub const MPV_EVENT_NONE: mpv_event_id = 0;
pub const MPV_EVENT_SHUTDOWN: mpv_event_id = 1;
pub const MPV_EVENT_START_FILE: mpv_event_id = 6;
pub const MPV_EVENT_END_FILE: mpv_event_id = 7;
pub const MPV_EVENT_FILE_LOADED: mpv_event_id = 8;
pub const MPV_EVENT_VIDEO_RECONFIG: mpv_event_id = 17;
pub const MPV_EVENT_SEEK: mpv_event_id = 20;
pub const MPV_EVENT_PLAYBACK_RESTART: mpv_event_id = 21;
pub const MPV_EVENT_PROPERTY_CHANGE: mpv_event_id = 22;

pub const MPV_END_FILE_REASON_EOF: c_int = 0;
pub const MPV_END_FILE_REASON_STOP: c_int = 2;
pub const MPV_END_FILE_REASON_QUIT: c_int = 3;
pub const MPV_END_FILE_REASON_ERROR: c_int = 4;
pub const MPV_END_FILE_REASON_REDIRECT: c_int = 5;

pub type mpv_render_param_type = c_int;
pub const MPV_RENDER_PARAM_INVALID: mpv_render_param_type = 0;
pub const MPV_RENDER_PARAM_API_TYPE: mpv_render_param_type = 1;
pub const MPV_RENDER_PARAM_ADVANCED_CONTROL: mpv_render_param_type = 10;
pub const MPV_RENDER_PARAM_BLOCK_FOR_TARGET_TIME: mpv_render_param_type = 12;
pub const MPV_RENDER_PARAM_SKIP_RENDERING: mpv_render_param_type = 13;
pub const MPV_RENDER_PARAM_SW_SIZE: mpv_render_param_type = 17;
pub const MPV_RENDER_PARAM_SW_FORMAT: mpv_render_param_type = 18;
pub const MPV_RENDER_PARAM_SW_STRIDE: mpv_render_param_type = 19;
pub const MPV_RENDER_PARAM_SW_POINTER: mpv_render_param_type = 20;

pub const MPV_RENDER_UPDATE_FRAME: u64 = 1;

#[repr(C)]
pub struct mpv_render_param {
    pub type_: mpv_render_param_type,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct mpv_event {
    pub event_id: mpv_event_id,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct mpv_event_property {
    pub name: *const c_char,
    pub format: mpv_format,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct mpv_event_end_file {
    pub reason: c_int,
    pub error: c_int,
    pub playlist_entry_id: i64,
    pub playlist_insert_id: i64,
    pub playlist_insert_num_entries: c_int,
}

#[repr(C)]
pub union mpv_node_u {
    pub string: *mut c_char,
    pub flag: c_int,
    pub int64: i64,
    pub double_: c_double,
    pub list: *mut mpv_node_list,
    pub ba: *mut c_void,
}

#[repr(C)]
pub struct mpv_node {
    pub u: mpv_node_u,
    pub format: mpv_format,
}

#[repr(C)]
pub struct mpv_node_list {
    pub num: c_int,
    pub values: *mut mpv_node,
    pub keys: *mut *mut c_char,
}

// On Linux the system libmpv (>= API 2.0, `libmpv.so.2`) provides these symbols. On Windows
// the installer ships `libmpv-2.dll`; its import library is found through MPV_LIB_DIR
// (see build.rs). The desktop app never links a static mpv.
#[cfg_attr(target_os = "windows", link(name = "mpv-2"))]
#[cfg_attr(not(target_os = "windows"), link(name = "mpv"))]
unsafe extern "C" {
    pub fn mpv_client_api_version() -> std::ffi::c_ulong;
    pub fn mpv_error_string(error: c_int) -> *const c_char;
    pub fn mpv_free(data: *mut c_void);
    pub fn mpv_create() -> *mut mpv_handle;
    pub fn mpv_initialize(ctx: *mut mpv_handle) -> c_int;
    pub fn mpv_terminate_destroy(ctx: *mut mpv_handle);
    pub fn mpv_set_option_string(
        ctx: *mut mpv_handle,
        name: *const c_char,
        data: *const c_char,
    ) -> c_int;
    pub fn mpv_command(ctx: *mut mpv_handle, args: *mut *const c_char) -> c_int;
    pub fn mpv_command_async(
        ctx: *mut mpv_handle,
        reply_userdata: u64,
        args: *mut *const c_char,
    ) -> c_int;
    pub fn mpv_set_property(
        ctx: *mut mpv_handle,
        name: *const c_char,
        format: mpv_format,
        data: *mut c_void,
    ) -> c_int;
    pub fn mpv_set_property_string(
        ctx: *mut mpv_handle,
        name: *const c_char,
        data: *const c_char,
    ) -> c_int;
    pub fn mpv_get_property(
        ctx: *mut mpv_handle,
        name: *const c_char,
        format: mpv_format,
        data: *mut c_void,
    ) -> c_int;
    pub fn mpv_get_property_string(ctx: *mut mpv_handle, name: *const c_char) -> *mut c_char;
    pub fn mpv_observe_property(
        mpv: *mut mpv_handle,
        reply_userdata: u64,
        name: *const c_char,
        format: mpv_format,
    ) -> c_int;
    pub fn mpv_wait_event(ctx: *mut mpv_handle, timeout: c_double) -> *mut mpv_event;
    pub fn mpv_wakeup(ctx: *mut mpv_handle);
    pub fn mpv_free_node_contents(node: *mut mpv_node);

    pub fn mpv_render_context_create(
        res: *mut *mut mpv_render_context,
        mpv: *mut mpv_handle,
        params: *mut mpv_render_param,
    ) -> c_int;
    pub fn mpv_render_context_set_update_callback(
        ctx: *mut mpv_render_context,
        callback: Option<unsafe extern "C" fn(*mut c_void)>,
        callback_ctx: *mut c_void,
    );
    pub fn mpv_render_context_update(ctx: *mut mpv_render_context) -> u64;
    pub fn mpv_render_context_render(
        ctx: *mut mpv_render_context,
        params: *mut mpv_render_param,
    ) -> c_int;
    pub fn mpv_render_context_free(ctx: *mut mpv_render_context);
}

/// Converts an mpv node tree into JSON. Strings that aren't UTF-8 are replaced lossily.
///
/// # Safety
/// `node` must be a valid node mpv filled in.
pub unsafe fn node_to_json(node: &mpv_node) -> serde_json::Value {
    use serde_json::Value;
    unsafe {
        match node.format {
            MPV_FORMAT_STRING => cstr(node.u.string)
                .map(Value::String)
                .unwrap_or(Value::Null),
            MPV_FORMAT_FLAG => Value::Bool(node.u.flag != 0),
            MPV_FORMAT_INT64 => Value::from(node.u.int64),
            MPV_FORMAT_DOUBLE => serde_json::Number::from_f64(node.u.double_)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            MPV_FORMAT_NODE_ARRAY => {
                if node.u.list.is_null()
                    || (*node.u.list).num <= 0
                    || (*node.u.list).values.is_null()
                {
                    return Value::Array(Vec::new());
                }
                let list = &*node.u.list;
                let values = std::slice::from_raw_parts(list.values, list.num.max(0) as usize);
                Value::Array(values.iter().map(|v| node_to_json(v)).collect())
            }
            MPV_FORMAT_NODE_MAP => {
                if node.u.list.is_null()
                    || (*node.u.list).num <= 0
                    || (*node.u.list).values.is_null()
                    || (*node.u.list).keys.is_null()
                {
                    return Value::Object(Default::default());
                }
                let list = &*node.u.list;
                let n = list.num.max(0) as usize;
                let values = std::slice::from_raw_parts(list.values, n);
                let keys = std::slice::from_raw_parts(list.keys, n);
                let mut map = serde_json::Map::new();
                for (k, v) in keys.iter().zip(values) {
                    if let Some(k) = cstr(*k) {
                        map.insert(k, node_to_json(v));
                    }
                }
                Value::Object(map)
            }
            _ => Value::Null,
        }
    }
}

/// # Safety
/// `p` must be null or a valid NUL-terminated string.
pub unsafe fn cstr(p: *const c_char) -> Option<String> {
    if p.is_null() {
        None
    } else {
        Some(
            unsafe { std::ffi::CStr::from_ptr(p) }
                .to_string_lossy()
                .into_owned(),
        )
    }
}
