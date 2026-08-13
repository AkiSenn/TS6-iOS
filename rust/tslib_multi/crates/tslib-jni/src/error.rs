use jni::JNIEnv;

const EXCEPTION_CLASS: &str = "dev/tslib/TsLibException";

/// Throw a `TsLibException` on the Java side.
pub fn throw_tslib_exception(env: &mut JNIEnv, msg: &str) {
    let _ = env.throw_new(EXCEPTION_CLASS, msg);
}

/// Convert a `tslib_core::Result<T>` into an `Option<T>`, throwing a Java
/// exception on error.
pub fn to_jni_result<T>(env: &mut JNIEnv, result: tslib_core::Result<T>) -> Option<T> {
    match result {
        Ok(val) => Some(val),
        Err(err) => {
            throw_tslib_exception(env, &err.to_string());
            None
        }
    }
}

/// Convert a `tslib_audio::Result<T>` into an `Option<T>`, throwing a Java
/// exception on error.
pub fn audio_to_jni_result<T>(
    env: &mut JNIEnv,
    result: tslib_audio::Result<T>,
) -> Option<T> {
    match result {
        Ok(val) => Some(val),
        Err(err) => {
            throw_tslib_exception(env, &err.to_string());
            None
        }
    }
}
