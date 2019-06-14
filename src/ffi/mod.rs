#![allow(dead_code)]
use libc::*;
use std::fs::File;
use std::os::unix::io::{AsRawFd, RawFd};
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

pub struct HookCommand {
    _hook: Hook,
    _callback: Box<Box<dyn FnMut(Buffer, &str)>>,
}

pub fn hook_command<F: FnMut(Buffer, &str) + 'static>(
    cmd: &str,
    desc: &str,
    args: &str,
    argdesc: &str,
    compl: &str,
    func: F,
) -> Option<HookCommand> {
    type CB = dyn FnMut(Buffer, &str);
    extern "C" {
        fn wdc_hook_command(
            command: *const c_char,
            description: *const c_char,
            args: *const c_char,
            args_description: *const c_char,
            completion: *const c_char,
            pointer: *const c_void,
            callback: extern "C" fn(
                *const c_void,
                *mut c_void,
                *mut c_void,
                c_int,
                *mut *mut c_char,
                *mut *mut c_char,
            ) -> c_int,
        ) -> *mut c_void;
    }
    extern "C" fn callback(
        pointer: *const c_void,
        data: *mut c_void,
        buffer: *mut c_void,
        argc: c_int,
        argv: *mut *mut c_char,
        argv_eol: *mut *mut c_char,
    ) -> c_int {
        let _ = data;
        let _ = argv;
        wrap_panic(|| {
            let pointer = pointer as *mut Box<CB>;
            let buffer = Buffer { ptr: buffer };
            if argc <= 1 {
                (unsafe { &mut **pointer })(buffer, "");
                return;
            }
            let args = unsafe { *argv_eol.offset(1) };
            let args = unsafe { CStr::from_ptr(args).to_str() };
            let args = match args {
                Ok(x) => x,
                Err(_) => return,
            };
            (unsafe { &mut **pointer })(buffer, args);
        });
        0
    }
    unsafe {
        let cmd = unwrap1!(CString::new(cmd));
        let desc = unwrap1!(CString::new(desc));
        let args = unwrap1!(CString::new(args));
        let argdesc = unwrap1!(CString::new(argdesc));
        let compl = unwrap1!(CString::new(compl));
        let custom_callback: Box<Box<CB>> = Box::new(Box::new(func));
        let pointer = &*custom_callback as *const _ as *const c_void;
        let hook = wdc_hook_command(
            cmd.as_ptr(),
            desc.as_ptr(),
            args.as_ptr(),
            argdesc.as_ptr(),
            compl.as_ptr(),
            pointer,
            callback,
        );
        if hook.is_null() {
            None
        } else {
            Some(HookCommand {
                _hook: Hook { ptr: hook },
                _callback: custom_callback,
            })
        }
    }
}

pub struct HookCommandRun {
    _hook: Hook,
    _callback: Box<Box<dyn FnMut(Buffer, &str) -> i32>>,
}

pub fn hook_command_run<F: FnMut(Buffer, &str) -> i32 + 'static>(
    cmd: &str,
    func: F,
) -> Option<HookCommandRun> {
    type CB = dyn FnMut(Buffer, &str) -> i32;
    extern "C" {
        fn wdc_hook_command_run(
            command: *const c_char,
            pointer: *const c_void,
            callback: extern "C" fn(*const c_void, *mut c_void, *mut c_void, *mut c_char) -> c_int,
        ) -> *mut c_void;
    }
    extern "C" fn callback(
        pointer: *const c_void,
        data: *mut c_void,
        buffer: *mut c_void,
        command: *mut c_char,
    ) -> c_int {
        let _ = data;
        wrap_panic(|| {
            let pointer = pointer as *mut Box<CB>;
            let buffer = Buffer { ptr: buffer };
            // TODO: Improve ignore ability
            // If it is not a weecord buffer, let the command pass through
            if buffer.get("localvar_guildid").is_none() {
                return WEECHAT_RC_OK;
            };

            let command = unsafe { CStr::from_ptr(command).to_str() };
            let command = match command {
                Ok(x) => x,
                Err(_) => return WEECHAT_RC_ERROR,
            };
            (unsafe { &mut **pointer })(buffer, command)
        })
        .unwrap_or(WEECHAT_RC_ERROR)
    }
    unsafe {
        let cmd = unwrap1!(CString::new(cmd));
        let custom_callback: Box<Box<CB>> = Box::new(Box::new(func));
        let pointer = &*custom_callback as *const _ as *const c_void;
        let hook = wdc_hook_command_run(cmd.as_ptr(), pointer, callback);
        if hook.is_null() {
            None
        } else {
            Some(HookCommandRun {
                _hook: Hook { ptr: hook },
                _callback: custom_callback,
            })
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

#[derive(Debug)]
pub enum SignalHookData {
    String(String),
    Integer(i32),
    Pointer(Buffer),
}

impl SignalHookData {
    pub fn from_value_data(data_type: &str, data: *mut c_void) -> Option<SignalHookData> {
        match data_type {
            "string" => {
                if let Ok(str) = unsafe { CStr::from_ptr(data as *const c_char).to_str() } {
                    Some(SignalHookData::String(str.to_owned()))
                } else {
                    None
                }
            }
            "integer" => {
                let data = data as *const c_int;
                if data.is_null() {
                    None
                } else {
                    unsafe { Some(SignalHookData::Integer(*(data))) }
                }
            }
            "pointer" => Some(SignalHookData::Pointer(Buffer::from_ptr(data))),
            _ => None,
        }
    }
}

pub struct SignalHook {
    _custom_callback: Box<Box<dyn FnMut(SignalHookData)>>,
    _hook: *mut c_void,
}

pub fn hook_signal<F: FnMut(SignalHookData) + 'static>(
    signal: &str,
    func: F,
) -> Option<SignalHook> {
    type CB = dyn FnMut(SignalHookData);
    extern "C" {
        fn wdc_hook_signal(
            signal: *const c_char,
            callback: extern "C" fn(
                *const c_void,
                *mut c_void,
                *const c_char,
                *const c_char,
                *mut c_void,
            ) -> c_int,
            pointer: *const c_void,
        ) -> *mut c_void;
    }
    extern "C" fn cb(
        pointer: *const c_void,
        _data: *mut c_void,
        _signal: *const c_char,
        type_data: *const c_char,
        signal_data: *mut c_void,
    ) -> c_int {
        let type_str = unsafe { CStr::from_ptr(type_data) };
        let type_str = match type_str.to_str() {
            Ok(s) => s,
            Err(_) => return 1,
        };

        if let Some(typed_data) = SignalHookData::from_value_data(type_str, signal_data) {
            let pointer = pointer as *mut Box<CB>;
            (unsafe { &mut **pointer })(typed_data);
        };
        0
    }

    let ptr = CString::new(signal).ok()?;
    let _custom_callback: Box<Box<CB>> = Box::new(Box::new(func));
    let pointer = &*_custom_callback as *const _ as *const c_void;
    let _hook = unsafe { wdc_hook_signal(ptr.as_ptr(), cb, pointer) };

    if _hook.is_null() {
        None
    } else {
        Some(SignalHook {
            _hook,
            _custom_callback,
        })
    }
}

pub struct FdHook {
    file: File,
    _callback: Box<Box<dyn FnMut(RawFd)>>,
    _hook: *mut c_void,
}

pub fn hook_fd<F: FnMut(RawFd) + 'static>(file: File, func: F) -> Option<FdHook> {
    type CB = dyn FnMut(RawFd);
    extern "C" {
        fn wdc_hook_fd(
            fd: c_int,
            pointer: *const c_void,
            callback: extern "C" fn(*const c_void, *mut c_void, c_int) -> c_int,
        ) -> *mut c_void;
    }
    extern "C" fn callback(pointer: *const c_void, _data: *mut c_void, fd: c_int) -> c_int {
        let pointer = pointer as *mut Box<CB>;
        (unsafe { &mut **pointer })(fd as RawFd);
        0
    }

    let _callback: Box<Box<CB>> = Box::new(Box::new(func));
    let pointer = &*_callback as *const _ as *const c_void;
    let fd = file.as_raw_fd();
    let _hook = unsafe { wdc_hook_fd(fd, pointer, callback) };

    if _hook.is_null() {
        None
    } else {
        Some(FdHook {
            file,
            _callback,
            _hook,
        })
    }
}

pub struct TimerHook {
    _callback: Box<Box<dyn FnMut(i32)>>,
    _hook: *mut c_void,
}

pub fn hook_timer<F: FnMut(i32) + 'static>(
    interval: i32,
    align_second: i32,
    max_calls: i32,
    func: F,
) -> Option<TimerHook> {
    type CB = dyn FnMut(i32);
    extern "C" {
        fn wdc_hook_timer(
            interval: c_int,
            align_second: c_int,
            max_calls: c_int,
            pointer: *const c_void,
            callback: extern "C" fn(*const c_void, *mut c_void, c_int) -> c_int,
        ) -> *mut c_void;
    }
    extern "C" fn callback(
        pointer: *const c_void,
        _data: *mut c_void,
        calls_remaining: c_int,
    ) -> c_int {
        let pointer = pointer as *mut Box<CB>;
        (unsafe { &mut **pointer })(calls_remaining as i32);
        0
    }

    let _callback: Box<Box<CB>> = Box::new(Box::new(func));
    let pointer = &*_callback as *const _ as *const c_void;
    let _hook = unsafe { wdc_hook_timer(interval, align_second, max_calls, pointer, callback) };

    if _hook.is_null() {
        None
    } else {
        Some(TimerHook { _callback, _hook })
    }
}

// TODO: Rework this binding?
impl Completion {
    pub fn add(&mut self, word: &str) {
        extern "C" {
            fn wdc_hook_completion_add(gui_completion: *const c_void, word: *const c_char);
        }
        unsafe {
            let word_c = CString::new(word).unwrap();
            wdc_hook_completion_add(self.ptr, word_c.as_ptr());
        }
    }
}

pub fn hook_completion<F: Fn(Buffer, &str, Completion) + 'static>(
    name: &str,
    description: &str,
    callback: F,
) -> Option<Hook> {
    type CB = dyn Fn(Buffer, &str, Completion);
    extern "C" {
        fn wdc_hook_completion(
            completion_item: *const c_char,
            description: *const c_char,
            callback_pointer: *const c_void,
            callback: extern "C" fn(
                *const c_void,
                *mut c_void,
                *const c_char,
                *mut c_void,
                *mut c_void,
            ) -> c_int,
        ) -> *mut c_void;
    }
    extern "C" fn callback_func(
        pointer: *const c_void,
        data: *mut c_void,
        completion_item: *const c_char,
        buffer: *mut c_void,
        completion: *mut c_void,
    ) -> c_int {
        let _ = data;
        wrap_panic(|| {
            let buffer = Buffer { ptr: buffer };
            let completion = Completion { ptr: completion };
            let pointer = pointer as *const Box<CB>;

            let completion_item = unsafe { CStr::from_ptr(completion_item) };
            let completion_item = match completion_item.to_str() {
                Ok(s) => s,
                Err(_) => return,
            };
            (unsafe { &**pointer })(buffer, completion_item, completion);
        });
        0
    }
    let callback: Box<Box<CB>> = Box::new(Box::new(callback));
    unsafe {
        let name_c = unwrap1!(CString::new(name));
        let description_c = unwrap1!(CString::new(description));
        let callback = Box::into_raw(callback) as *const _ as *const c_void; // TODO: Memory leak
        let result = wdc_hook_completion(
            name_c.as_ptr(),
            description_c.as_ptr(),
            callback,
            callback_func,
        );
        if result.is_null() {
            None
        } else {
            Some(Hook { ptr: result })
        }
    }
}
