# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile

-keep class chat.kursal.BleAdvertiser { *; }
-keepclassmembers class chat.kursal.BleAdvertiser {
    public static *** start(android.content.Context, java.lang.String, java.lang.String, java.lang.String);
    public static *** stop(android.content.Context);
    public static native ** nativeOnReadRequest();
    public static native ** nativeOnWriteRequest(java.lang.String, byte[]);
}

# droidplug: btleplug's Android backend, vendored under src/main/java (see the
# README there). Nothing on the Java side references these classes -- btleplug's
# platform::init() looks them up by name over JNI -- so R8 would otherwise strip
# them and every BLE call would abort the process.
-keep class com.nonpolynomial.btleplug.** { *; }
-keep class io.github.gedgygedgy.rust.** { *; }

# Window insets bridge; only ever called from the WebView's JS context.
-keep class chat.kursal.MainActivity$InsetsBridge { *; }