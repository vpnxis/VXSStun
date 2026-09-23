package app.vxsstun.client

import org.json.JSONArray
import org.json.JSONObject

internal object XrayConfig {
    fun request(method: String, payload: JSONObject = JSONObject()): String =
        JSONObject().put("apiVersion", 1).put("method", method).put("payload", payload).toString()

    fun startRequest(config: JSONObject): String =
        request("runXrayFromJson", JSONObject().put("configJSON", config.toString()))

    fun build(outbound: JSONObject, tunFd: Int, metrics: Int, health: Int): JSONObject {
        require(tunFd >= 0 && metrics in 1..65535 && health in 1..65535 && metrics != health)
        val tun = JSONObject().put("tag", "tun").put("port", 0).put("protocol", "tun")
            .put("settings", JSONObject().put("name", "vxsstun").put("mtu", 1400))
        val check = JSONObject().put("tag", "health-http").put("listen", "127.0.0.1")
            .put("port", health).put("protocol", "http").put("settings", JSONObject())
        return JSONObject()
            .put("log", JSONObject().put("loglevel", "none"))
            .put("env", JSONObject().put("xray.tun.fd", tunFd.toString()))
            .put("inbounds", JSONArray().put(tun).put(check))
            .put("outbounds", JSONArray().put(JSONObject(outbound.toString())))
            .put("stats", JSONObject())
            .put("policy", JSONObject().put("system", JSONObject()
                .put("statsOutboundUplink", true).put("statsOutboundDownlink", true)))
            .put("metrics", JSONObject().put("listen", "127.0.0.1:$metrics"))
    }
}
