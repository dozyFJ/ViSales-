use jni::{
    JavaVM, jni_sig, jni_str,
    objects::{JObject, JString},
};
use winit::platform::android::activity::AndroidApp;

pub fn start(app: &AndroidApp) -> jni::errors::Result<()> {
    // AndroidApp holds the Java VM and global Activity reference for this lifecycle.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let raw = app.activity_as_ptr().cast();
    vm.attach_current_thread(|env| {
        let activity = unsafe { env.as_cast_raw::<JObject>(&raw)? };
        env.call_method(
            &activity,
            jni_str!("startBarcodeScan"),
            jni_sig!("()V"),
            &[],
        )?;
        Ok(())
    })
}

pub fn take(app: &AndroidApp) -> jni::errors::Result<Option<String>> {
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let raw = app.activity_as_ptr().cast();
    vm.attach_current_thread(|env| {
        let activity = unsafe { env.as_cast_raw::<JObject>(&raw)? };
        let value = env
            .call_method(
                &activity,
                jni_str!("takeBarcodeScanResult"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            )?
            .l()?;
        if value.is_null() {
            Ok(None)
        } else {
            Ok(Some(env.cast_local::<JString>(value)?.try_to_string(env)?))
        }
    })
}

pub fn check_updates(app: &AndroidApp) -> jni::errors::Result<()> {
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let raw = app.activity_as_ptr().cast();
    vm.attach_current_thread(|env| {
        let activity = unsafe { env.as_cast_raw::<JObject>(&raw)? };
        env.call_method(
            &activity,
            jni_str!("checkViSalesUpdates"),
            jni_sig!("()V"),
            &[],
        )?;
        Ok(())
    })
}
