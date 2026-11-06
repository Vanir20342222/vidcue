//! Android-only JNI glue: the `MediaBridge` class handle the engine calls for the
//! isolated JavaScript solver and Keystore account sealing. Screens are native Compose.
use super::*;
use jni::{
    JavaVM,
    objects::{GlobalRef, JClass, JValue},
};
static BRIDGE: OnceLock<(JavaVM, GlobalRef)> = OnceLock::new();

pub(crate) fn initialize_bridge(env: &mut jni::JNIEnv) -> Result<()> {
    if BRIDGE.get().is_none() {
        let vm = env.get_java_vm()?;
        let class = env.find_class("dev/ytui/android/MediaBridge")?;
        let global = env.new_global_ref(class)?;
        let _ = BRIDGE.set((vm, global));
    }
    yt_dlp_rs::set_embedded_js_evaluator(|script| {
        let result = (|| -> Result<String> {
            let (vm, class) = BRIDGE.get().context("Missing Android JS bridge")?;
            let mut env = vm.attach_current_thread()?;
            let class: &JClass = class.as_obj().into();
            let text = env.new_string(script)?;
            let value = match env.call_static_method(
                class,
                "evaluate",
                "(Ljava/lang/String;)Ljava/lang/String;",
                &[JValue::Object(&text)],
            ) {
                Ok(value) => value.l()?,
                Err(error) => {
                    let _ = env.exception_clear();
                    return Err(error.into());
                }
            };
            let result: String = env.get_string((&value).into())?.into();
            Ok(result)
        })();
        result.map_err(|_| "Android isolated JavaScript solver failed or timed out".into())
    });
    Ok(())
}

pub(crate) fn account_cipher(method: &str, text: &str) -> Result<String> {
    let result = (|| -> Result<String> {
        let (vm, class) = BRIDGE.get().context("Missing Android bridge")?;
        let mut env = vm.attach_current_thread()?;
        let class: &JClass = class.as_obj().into();
        let input = env.new_string(text)?;
        let value = match env.call_static_method(
            class,
            method,
            "(Ljava/lang/String;)Ljava/lang/String;",
            &[JValue::Object(&input)],
        ) {
            Ok(v) => v.l()?,
            Err(e) => {
                let _ = env.exception_clear();
                return Err(e.into());
            }
        };
        Ok(env.get_string((&value).into())?.into())
    })();
    // Keystore errors can contain implementation data. Never include the input,
    // JNI exception text or serialized session in startup/import diagnostics.
    result.map_err(|_| {
        anyhow::anyhow!("Android Keystore could not access the account; reconnect or disconnect it")
    })
}
