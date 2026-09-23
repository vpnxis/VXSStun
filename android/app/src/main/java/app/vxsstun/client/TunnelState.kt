package app.vxsstun.client
import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
object TunnelState {
    @Volatile var status="disconnected"
    @Volatile var profileId:String?=null
    @Volatile var error:String?=null
    @Volatile var connectedAt:Long?=null
    @Volatile var up:Long?=null
    @Volatile var down:Long?=null
    @Volatile var reconnects=0
    @Volatile var store:ProfileStore?=null
    private val events=ArrayDeque<JSONObject>()
    @Synchronized fun storage(context:Context):ProfileStore = store?:ProfileStore(context.applicationContext).also {store=it}
    fun active()=status in listOf("connecting","connected","reconnecting","disconnecting")
    @Synchronized fun event(message:String){events.addLast(JSONObject().put("at",System.currentTimeMillis()/1000).put("message",message));while(events.size>100)events.removeFirst()}
    @Synchronized fun snapshot(context:Context):JSONObject {
        val db=storage(context)
        return JSONObject().put("profiles",db.summaries()).put("subscriptions",db.subscriptions()).put("settings",db.settings()).put("version",BuildConfig.VERSION_NAME).put("coreVersion","Xray / libXray 26.7.28").put("elevated",true)
            .put("connection",JSONObject().put("status",status).put("profileId",profileId?:JSONObject.NULL).put("error",error?:JSONObject.NULL).put("connectedAt",connectedAt?:JSONObject.NULL).put("bytesUp",up?:JSONObject.NULL).put("bytesDown",down?:JSONObject.NULL).put("reconnects",reconnects).put("mode","tun"))
            .put("events",JSONArray(events.toList()))
    }
}
