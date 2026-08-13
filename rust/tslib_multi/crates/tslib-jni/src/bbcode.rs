use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;

use tslib_chat::bbcode::{strip_bbcode, BBCodeParser};

use crate::require_string;

/// `BBCode.toHtml(input)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_tslib_BBCode_toHtml(
    mut env: JNIEnv,
    _class: JClass,
    input: JString,
) -> jstring {
    let input = match require_string(&mut env, &input) {
        Ok(s) => s,
        Err(()) => return std::ptr::null_mut(),
    };
    let result = BBCodeParser::to_html(&input);
    env.new_string(&result)
        .map(|js| js.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// `BBCode.toPlain(input)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_tslib_BBCode_toPlain(
    mut env: JNIEnv,
    _class: JClass,
    input: JString,
) -> jstring {
    let input = match require_string(&mut env, &input) {
        Ok(s) => s,
        Err(()) => return std::ptr::null_mut(),
    };
    let result = BBCodeParser::to_plain(&input);
    env.new_string(&result)
        .map(|js| js.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// `BBCode.toAnsi(input)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_tslib_BBCode_toAnsi(
    mut env: JNIEnv,
    _class: JClass,
    input: JString,
) -> jstring {
    let input = match require_string(&mut env, &input) {
        Ok(s) => s,
        Err(()) => return std::ptr::null_mut(),
    };
    let result = BBCodeParser::to_ansi(&input);
    env.new_string(&result)
        .map(|js| js.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// `BBCode.strip(input)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_tslib_BBCode_strip(
    mut env: JNIEnv,
    _class: JClass,
    input: JString,
) -> jstring {
    let input = match require_string(&mut env, &input) {
        Ok(s) => s,
        Err(()) => return std::ptr::null_mut(),
    };
    let result = strip_bbcode(&input);
    env.new_string(&result)
        .map(|js| js.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
