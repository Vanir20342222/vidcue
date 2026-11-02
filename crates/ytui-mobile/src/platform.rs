//! Android JNI adapters for the native Compose application.
use super::*;
use jni::{
    JavaVM,
    objects::{GlobalRef, JClass, JValue},
};

#[cfg(feature = "legacy-ui")]
mod legacy;
#[cfg(feature = "legacy-ui")]
pub use legacy::paint_video;

static BRIDGE: OnceLock<(JavaVM, GlobalRef)> = OnceLock::new();

#[cfg(feature = "legacy-ui")]
static CONTEXT: std::sync::Mutex<Option<egui::Context>> = std::sync::Mutex::new(None);
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ytui_android_NativeBridge_frameAvailable(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) {
    #[cfg(feature = "legacy-ui")]
    if let Some(ctx) = CONTEXT.lock().expect("context").as_ref() {
        ctx.request_repaint();
    }
}
static UI_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static UI_SHUTDOWNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ytui_android_NativeBridge_uiShutdowns(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) -> jni::sys::jlong {
    UI_SHUTDOWNS.load(std::sync::atomic::Ordering::Relaxed) as jni::sys::jlong
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ytui_android_NativeBridge_uiGeneration(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) -> jni::sys::jlong {
    UI_GENERATION.load(std::sync::atomic::Ordering::Relaxed) as jni::sys::jlong
}

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
