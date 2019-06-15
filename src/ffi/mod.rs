#![allow(dead_code)]
use libc::*;
use std::{
    ffi::{CStr, CString},
    panic::{catch_unwind, UnwindSafe},
    ptr,
};

#[macro_use]
mod macros;

const WEECHAT_RC_ERROR: i32 = -1;
const WEECHAT_RC_OK: i32 = 0;
const WEECHAT_RC_OK_EAT: i32 = 1;

#[derive(PartialEq, Eq, Hash, Debug)]
pub struct Buffer {
    ptr: *mut c_void,
}

unsafe impl Send for Buffer {}

pub const MAIN_BUFFER: Buffer = Buffer {
    ptr: ptr::null_mut(),
};

pub struct Completion {
    ptr: *mut c_void,
}

pub struct Hook {
    ptr: *mut c_void,
}

impl Drop for Hook {
    fn drop(&mut self) {
        extern "C" {
            fn wdc_unhook(hook: *mut c_void);
        }
        unsafe {
            wdc_unhook(self.ptr);
        }
    }
}

pub struct WeechatAny {
    data: *mut c_void,
    hdata: *mut c_void,
}

impl PartialEq for WeechatAny {
    fn eq(&self, rhs: &Self) -> bool {
        self.ptr() == rhs.ptr()
    }
}

pub struct SharedString(pub String);

fn strip_indexer_field(field: &str) -> &str {
    if let Some(idx) = field.find('|') {
        &field[(idx + 1)..]
    } else {
        field
    }
}

pub trait WeechatObject {
    fn from_ptr_hdata(ptr: *mut c_void, hdata: *mut c_void) -> Self;
    fn ptr(&self) -> *mut c_void;
    fn hdata(&self) -> *mut c_void;

    fn get<T: HDataGetResult>(&self, field: &str) -> Option<T> {
        let field_type = T::weechat_type();
        if field_type != "" {
            let actual_type = hdata_get_var_type_string(self.hdata(), field);
            if field_type != actual_type {
                really_bad(format!(
                    "Field {} had type {} but we expected {}",
                    field, actual_type, field_type
                ));
            }
        }
        T::new::<Self>(self, field)
    }

    fn get_idx<T: HDataGetResult>(&self, field: &str, index: usize) -> Option<T> {
        self.get(&format!("{}|{}", index, field))
    }

    fn get_any(&self, field: &str) -> Option<WeechatAny> {
        self.get(field)
    }
}

impl WeechatObject for WeechatAny {
    fn from_ptr_hdata(data: *mut c_void, hdata: *mut c_void) -> Self {
        WeechatAny { data, hdata }
    }

    fn ptr(&self) -> *mut c_void {
        self.data
    }

    fn hdata(&self) -> *mut c_void {
        self.hdata
    }
}

pub trait HDataGetResult: Sized {
    fn new<T: WeechatObject + ?Sized>(parent: &T, field: &str) -> Option<Self>;
    fn weechat_type() -> &'static str;
}

impl<T: WeechatObject> HDataGetResult for T {
    fn new<P: WeechatObject + ?Sized>(parent: &P, field: &str) -> Option<Self> {
        let data = hdata_pointer(parent.hdata(), parent.ptr(), field);
        let data = match data {
            Some(data) => data,
            None => return None,
        };
        let hdata_name = hdata_get_var_hdata(parent.hdata(), field);
        let hdata = hdata_get(&hdata_name);
        Some(Self::from_ptr_hdata(data, hdata))
    }

    fn weechat_type() -> &'static str {
        "pointer"
    }
}

impl HDataGetResult for String {
    fn new<P: WeechatObject + ?Sized>(parent: &P, field: &str) -> Option<Self> {
        hdata_string(parent.hdata(), parent.ptr(), field)
    }

    fn weechat_type() -> &'static str {
        "string"
    }
}

impl HDataGetResult for SharedString {
    fn new<P: WeechatObject + ?Sized>(parent: &P, field: &str) -> Option<Self> {
        hdata_string(parent.hdata(), parent.ptr(), field).map(SharedString)
    }

    fn weechat_type() -> &'static str {
        "shared_string"
    }
}

impl HDataGetResult for i32 {
    fn new<P: WeechatObject + ?Sized>(parent: &P, field: &str) -> Option<Self> {
        hdata_integer(parent.hdata(), parent.ptr(), field).map(|x| x as i32)
    }

    fn weechat_type() -> &'static str {
        "integer"
    }
}

pub fn really_bad(message: String) -> ! {
    crate::plugin_print(&format!("Internal error - {}", message));
    panic!(message); // hopefully we hit a catch_unwind
}

pub fn get_plugin() -> *mut c_void {
    extern "C" {
        fn get_plugin() -> *mut c_void;
    }
    unsafe { get_plugin() }
}

pub fn set_plugin(plugin: *mut c_void) {
    extern "C" {
        fn set_plugin(plugin: *mut c_void);
    }
    unsafe { set_plugin(plugin) }
}

impl Buffer {
    pub fn from_ptr(ptr: *mut c_void) -> Buffer {
        Buffer { ptr }
    }

    pub fn new(name: &str, on_input: fn(Buffer, &str)) -> Option<Buffer> {
        extern "C" {
            fn wdc_buffer_new(
                name: *const c_char,
                pointer: *const c_void,
                input_callback: extern "C" fn(
                    *const c_void,
                    *mut c_void,
                    *mut c_void,
                    *const c_char,
                ) -> c_int,
                close_callback: extern "C" fn(*const c_void, *mut c_void, *mut c_void) -> c_int,
            ) -> *mut c_void;
        }
        extern "C" fn input_cb(
            pointer: *const c_void,
            data: *mut c_void,
            buffer: *mut c_void,
            input_data: *const c_char,
        ) -> c_int {
            let _ = data;
            wrap_panic(|| {
                let buffer = Buffer { ptr: buffer };
                let on_input: fn(Buffer, &str) = unsafe { ::std::mem::transmute(pointer) };
                let input_data = unsafe { CStr::from_ptr(input_data).to_str() };
                let input_data = match input_data {
                    Ok(x) => x,
                    Err(_) => return,
                };
                on_input(buffer, input_data);
            });
            0
        }
        extern "C" fn close_cb(
            pointer: *const c_void,
            data: *mut c_void,
            buffer: *mut c_void,
        ) -> c_int {
            let _ = pointer;
            let _ = data;
            let _ = buffer;
            0
        }
        unsafe {
            let name = unwrap1!(CString::new(name));
            let pointer = on_input as *const c_void;
            let result = wdc_buffer_new(name.as_ptr(), pointer, input_cb, close_cb);
            if result.is_null() {
                None
            } else {
                Some(Buffer { ptr: result })
            }
        }
    }

    pub fn search(name: &str) -> Option<Buffer> {
        extern "C" {
            fn wdc_buffer_search(name: *const c_char) -> *mut c_void;
        }
        unsafe {
            let name_c = unwrap1!(CString::new(name));
            let result = wdc_buffer_search(name_c.as_ptr());
            if result.is_null() {
                None
            } else {
                Some(Buffer { ptr: result })
            }
        }
    }

    pub fn current() -> Option<Buffer> {
        extern "C" {
            fn wdc_current_buffer() -> *mut c_void;
        }
        let result = unsafe { wdc_current_buffer() };
        if result.is_null() {
            None
        } else {
            Some(Buffer { ptr: result })
        }
    }

    pub fn print(&self, message: &str) {
        extern "C" {
            fn wdc_print(buffer: *mut c_void, message: *const c_char);
        }
        unsafe {
            let msg = unwrap1!(CString::new(message));
            wdc_print(self.ptr, msg.as_ptr());
        }
    }

    pub fn print_tags(&self, tags: &str, message: &str) {
        self.print_tags_dated(0, tags, message);
    }

    pub fn print_tags_dated(&self, date: i32, tags: &str, message: &str) {
        extern "C" {
            fn wdc_print_tags(
                buffer: *mut c_void,
                date: c_int,
                tags: *const c_char,
                message: *const c_char,
            );
        }
        unsafe {
            // let msg = unwrap1!(CString::new(message));
            let tags = unwrap1!(CString::new(tags));
            // In the event we get an error, try wiping all the nulls and trying again
            let msg = match CString::new(message) {
                Ok(msg) => msg,
                Err(_) => unwrap1!(CString::new(message.replace("\0", ""))),
            };
            wdc_print_tags(self.ptr, date, tags.as_ptr(), msg.as_ptr());
        }
    }

    /*
    pub fn load_weechat_backlog(&self) {
        extern "C" {
            fn wdc_load_backlog(sig_data: *mut c_void);
        }
        unsafe {
            wdc_load_backlog(self.ptr as *mut c_void);
        }
    }
    */

    pub fn send_trigger_hook(&self, hook_name: &str) {
        extern "C" {
            fn wdc_hook_signal_notify(sig_data: *mut c_void, signal: *const c_char);
        }
        unsafe {
            let hook_name = unwrap1!(CString::new(hook_name));
            wdc_hook_signal_notify(self.ptr as *mut c_void, hook_name.as_ptr());
        }
    }

    pub fn set(&self, property: &str, value: &str) {
        extern "C" {
            fn wdc_buffer_set(buffer: *mut c_void, property: *const c_char, value: *const c_char);
        }
        unsafe {
            let property = unwrap1!(CString::new(property));
            let value = unwrap1!(CString::new(value));
            wdc_buffer_set(self.ptr, property.as_ptr(), value.as_ptr());
        }
    }

    pub fn get(&self, property: &str) -> Option<String> {
        extern "C" {
            fn wdc_buffer_get(buffer: *mut c_void, property: *const c_char) -> *const c_char;
        }
        unsafe {
            let property = unwrap1!(CString::new(property));
            let value = wdc_buffer_get(self.ptr, property.as_ptr());
            if value.is_null() {
                None
            } else {
                Some(CStr::from_ptr(value).to_string_lossy().into_owned())
            }
        }
    }

    pub fn clear(&self) {
        extern "C" {
            fn wdc_buffer_clear(buffer: *mut c_void);
        }
        unsafe {
            wdc_buffer_clear(self.ptr);
        }
    }

    pub fn group_exists(&self, group: &str) -> bool {
        extern "C" {
            fn wdc_nicklist_group_exists(buffer: *const c_void, group: *const c_char) -> c_int;
        }
        unsafe {
            let group = CString::new(group).unwrap();
            wdc_nicklist_group_exists(self.ptr, group.as_ptr()) != 0
        }
    }

    /*
    pub fn nick_exists(&self, nick: &str) -> bool {
        extern "C" {
            fn wdc_nicklist_nick_exists(buffer: *const c_void, nick: *const c_char) -> c_int;
        }
        unsafe {
            let nick = CString::new(nick).unwrap();
            wdc_nicklist_nick_exists(self.ptr, nick.as_ptr()) != 0
        }
    }
    */

    pub fn add_nick(&self, nick: &str) {
        extern "C" {
            fn wdc_nicklist_add_nick(buffer: *const c_void, nick: *const c_char);
        }
        unsafe {
            let nick = CString::new(nick).unwrap();
            wdc_nicklist_add_nick(self.ptr, nick.as_ptr());
        }
    }

    pub fn add_nicklist_group(&self, name: &str) {
        self.add_nicklist_group_with_color(name, "weechat.color.nicklist_group")
    }

    pub fn add_nicklist_group_with_color(&self, name: &str, color: &str) {
        extern "C" {
            fn wdc_nicklist_add_group(
                buffer: *const c_void,
                name: *const c_char,
                color: *const c_char,
            );
        }
        unsafe {
            let name = CString::new(name).unwrap();
            let color = CString::new(color).unwrap();
            wdc_nicklist_add_group(self.ptr, name.as_ptr(), color.as_ptr());
        }
    }

    pub fn add_nick_to_group(&self, nick: &str, group: &str) {
        extern "C" {
            fn wdc_nicklist_add_nick_to_group(
                buffer: *const c_void,
                group: *const c_char,
                nick: *const c_char,
            );
        }
        unsafe {
            let nick = CString::new(nick).unwrap();
            let group = CString::new(group).unwrap();
            wdc_nicklist_add_nick_to_group(self.ptr, group.as_ptr(), nick.as_ptr());
        }
    }

    pub fn remove_nick(&self, nick: &str) {
        extern "C" {
            fn wdc_nicklist_remove_nick(buffer: *const c_void, nick: *const c_char);
        }
        unsafe {
            let nick = CString::new(nick).unwrap();
            wdc_nicklist_remove_nick(self.ptr, nick.as_ptr());
        }
    }
}

impl WeechatObject for Buffer {
    fn from_ptr_hdata(ptr: *mut c_void, hdata: *mut c_void) -> Self {
        let result = Buffer { ptr };
        if hdata != result.hdata() {
            really_bad("Buffer hdata pointer was different!".into());
        };
        result
    }

    fn ptr(&self) -> *mut c_void {
        self.ptr
    }

    fn hdata(&self) -> *mut c_void {
        hdata_get("buffer")
    }
}

fn wrap_panic<R, F: FnOnce() -> R + UnwindSafe>(f: F) -> Option<R> {
    let result = catch_unwind(f);
    match result {
        Ok(x) => Some(x),
        Err(err) => {
            let msg = match err.downcast_ref::<String>() {
                Some(msg) => msg,
                None => "unknown error",
            };
            let result =
                catch_unwind(|| crate::plugin_print(&format!("Fatal error (caught) - {}", msg)));
            let _ = result; // eat error without logging :(
            None
        }
    }
}

pub fn get_prefix(prefix: &str) -> Option<String> {
    extern "C" {
        fn wdc_prefix(prefix: *const c_char) -> *const c_char;
    }
    unsafe {
        let prefix = unwrap1!(CString::new(prefix));
        let value = wdc_prefix(prefix.as_ptr());
        if value.is_null() {
            None
        } else {
            Some(CStr::from_ptr(value).to_string_lossy().into_owned())
        }
    }
}

pub fn info_get(info_name: &str, arguments: &str) -> Option<String> {
    extern "C" {
        fn wdc_info_get(info_name: *const c_char, arguments: *const c_char) -> *const c_char;
    }
    unsafe {
        let info_name = unwrap1!(CString::new(info_name));
        let arguments = unwrap1!(CString::new(arguments));
        let result = wdc_info_get(info_name.as_ptr(), arguments.as_ptr());
        if result.is_null() {
            None
        } else {
            Some(CStr::from_ptr(result).to_string_lossy().into_owned())
        }
    }
}

fn hdata_get(name: &str) -> *mut c_void {
    extern "C" {
        fn wdc_hdata_get(name: *const c_char) -> *mut c_void;
    }
    unsafe {
        let name_c = unwrap1!(CString::new(name));
        let data = wdc_hdata_get(name_c.as_ptr());
        if data.is_null() {
            really_bad(format!("hdata name {} was invalid", name));
        }
        data
    }
}

fn hdata_pointer(hdata: *mut c_void, obj: *mut c_void, name: &str) -> Option<*mut c_void> {
    extern "C" {
        fn wdc_hdata_pointer(
            hdata: *mut c_void,
            obj: *mut c_void,
            name: *const c_char,
        ) -> *mut c_void;
    }
    unsafe {
        let name = unwrap1!(CString::new(name));
        let result = wdc_hdata_pointer(hdata, obj, name.as_ptr());
        if result.is_null() {
            None
        } else {
            Some(result)
        }
    }
}

fn hdata_get_var_hdata(hdata: *mut c_void, name: &str) -> String {
    extern "C" {
        fn wdc_hdata_get_var_hdata(hdata: *mut c_void, name: *const c_char) -> *const c_char;
    }
    let name = strip_indexer_field(name);
    unsafe {
        let name_c = unwrap1!(CString::new(name));
        let result = wdc_hdata_get_var_hdata(hdata, name_c.as_ptr());
        if result.is_null() {
            really_bad(format!("hdata field {} hdata was invalid", name));
        }
        CStr::from_ptr(result).to_string_lossy().into_owned()
    }
}

fn hdata_get_var_type_string(hdata: *mut c_void, name: &str) -> String {
    extern "C" {
        fn wdc_hdata_get_var_type_string(hdata: *mut c_void, name: *const c_char) -> *const c_char;
    }
    let name = strip_indexer_field(name);
    unsafe {
        let name_c = unwrap1!(CString::new(name));
        let result = wdc_hdata_get_var_type_string(hdata, name_c.as_ptr());
        if result.is_null() {
            really_bad(format!("hdata field {} type was invalid", name));
        }
        CStr::from_ptr(result).to_string_lossy().into_owned()
    }
}

fn hdata_integer(hdata: *mut c_void, data: *mut c_void, name: &str) -> Option<c_int> {
    extern "C" {
        fn wdc_hdata_integer(hdata: *mut c_void, data: *mut c_void, name: *const c_char) -> c_int;
    }
    unsafe {
        let name = unwrap1!(CString::new(name));
        Some(wdc_hdata_integer(hdata, data, name.as_ptr()))
    }
}

fn hdata_string(hdata: *mut c_void, data: *mut c_void, name: &str) -> Option<String> {
    extern "C" {
        fn wdc_hdata_string(
            hdata: *mut c_void,
            data: *mut c_void,
            name: *const c_char,
        ) -> *const c_char;
    }
    unsafe {
        let name = unwrap1!(CString::new(name));
        let result = wdc_hdata_string(hdata, data, name.as_ptr());
        if result.is_null() {
            None
        } else {
            Some(CStr::from_ptr(result).to_string_lossy().into_owned())
        }
    }
}

pub fn get_option(name: &str) -> Option<String> {
    extern "C" {
        fn wdc_config_get_plugin(name: *const c_char) -> *const c_char;
    }
    unsafe {
        let name_c = unwrap1!(CString::new(name));
        let result = wdc_config_get_plugin(name_c.as_ptr());
        if result.is_null() {
            None
        } else {
            Some(unwrap1!(CStr::from_ptr(result).to_str()).into())
        }
    }
}

pub fn set_option(name: &str, value: &str) -> String {
    extern "C" {
        fn wdc_config_set_plugin(name: *const c_char, value: *const c_char) -> c_int;
    }
    let before = get_option(name);
    let result = unsafe {
        let name_c = unwrap1!(CString::new(name));
        let value_c = unwrap1!(CString::new(value));
        wdc_config_set_plugin(name_c.as_ptr(), value_c.as_ptr())
    };
    match (result, before) {
        (0, Some(before)) => format!(
            "option {} successfully changed from {} to {}",
            name, before, value
        ),
        (0, None) | (1, None) => format!("option {} successfully set to {}", name, value),
        (1, Some(before)) => format!("option {} already contained {}", name, before),
        (2, _) => format!("option {} not found", name),
        (_, Some(before)) => format!(
            "error when setting option {} to {} (was {})",
            name, value, before
        ),
        (_, None) => format!("error when setting option {} to {}", name, value),
    }
}

pub fn remove_color(string: &str) -> String {
    extern "C" {
        fn wdc_string_remove_color(string: *const c_char) -> *mut c_char;
    }
    unsafe {
        let string_c = unwrap1!(CString::new(string));
        let result_c = wdc_string_remove_color(string_c.as_ptr());
        let result = CStr::from_ptr(result_c).to_str().unwrap().into();
        free(result_c as *mut c_void);
        result
    }
}

pub fn color_codes(color_name: &str) -> String {
    extern "C" {
        fn wdc_color(string: *const c_char) -> *const c_char;
    }
    unsafe {
        let string_c = unwrap1!(CString::new(color_name));
        let result_c = wdc_color(string_c.as_ptr());
        CStr::from_ptr(result_c).to_str().unwrap().into()
    }
}

pub fn update_bar_item(item_name: &str) {
    extern "C" {
        fn wdc_bar_item_update(string: *const c_char);
    }
    unsafe {
        let string_c = unwrap1!(CString::new(item_name));
        wdc_bar_item_update(string_c.as_ptr());
    }
}
