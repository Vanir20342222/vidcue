# Rust exports Java_dev_ytui_android_NativeBridge_* symbols by these exact names.
-keep class dev.ytui.android.NativeBridge { *; }

# Rust finds this class and calls its public static adapters by string name.
# This includes the isolated JS evaluator and encrypted account storage.
-keep class dev.ytui.android.MediaBridge {
    public static <methods>;
}

# WorkManager persists worker class names across app updates.
-keepnames class dev.ytui.android.PrepareWorker
