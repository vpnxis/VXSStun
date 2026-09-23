package app.vxsstun.client
object NativeProfiles {
    init { System.loadLibrary("vxsstun_profiles") }
    external fun parseImport(text: String): String
    external fun updateSubscription(data: String, request: String): String
}
