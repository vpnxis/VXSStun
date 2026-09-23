package app.vxsstun.client

import org.junit.Assert.*
import org.junit.Test
import org.json.JSONObject

class XrayConfigTest {
    private fun config() = XrayConfig.build(JSONObject().put("tag", "proxy").put("protocol", "socks"), 42, 41001, 41002)

    @Test fun usesExactLibXrayWireContract() {
        val request = JSONObject(XrayConfig.startRequest(config()))
        assertEquals(1, request.getInt("apiVersion"))
        assertEquals("runXrayFromJson", request.getString("method"))
        assertTrue(request.getJSONObject("payload").has("configJSON"))
        assertFalse(request.getJSONObject("payload").has("configJson"))
    }

    @Test fun usesProvidedFdAndLoopbackOnly() {
        val c = config()
        assertEquals("42", c.getJSONObject("env").getString("xray.tun.fd"))
        assertEquals("127.0.0.1:41001", c.getJSONObject("metrics").getString("listen"))
        assertEquals("127.0.0.1", c.getJSONArray("inbounds").getJSONObject(1).getString("listen"))
        assertEquals("tun", c.getJSONArray("inbounds").getJSONObject(0).getString("protocol"))
    }

    @Test fun noFallbackOrEmbeddedServers() {
        val c = config()
        assertEquals(1, c.getJSONArray("outbounds").length())
        assertEquals("socks", c.getJSONArray("outbounds").getJSONObject(0).getString("protocol"))
        assertFalse(c.has("routing"))
    }

    @Test fun trafficMetricsEnabled() {
        val s = config().getJSONObject("policy").getJSONObject("system")
        assertTrue(s.getBoolean("statsOutboundUplink"))
        assertTrue(s.getBoolean("statsOutboundDownlink"))
    }

    @Test(expected = IllegalArgumentException::class)
    fun rejectsCollidingPorts() { XrayConfig.build(JSONObject(), 1, 41000, 41000) }

    @Test(expected = IllegalArgumentException::class)
    fun rejectsClosedFd() { XrayConfig.build(JSONObject(), -1, 41000, 41001) }

    @Test fun preservesHysteriaTransportAndAuthentication() {
        val out=JSONObject("""{"tag":"proxy","protocol":"hysteria","settings":{"version":2,"address":"example.invalid","port":443},"streamSettings":{"network":"hysteria","security":"tls","hysteriaSettings":{"version":2,"auth":"test-only"}}}""")
        val copy=XrayConfig.build(out,42,41001,41002).getJSONArray("outbounds").getJSONObject(0)
        assertEquals("hysteria",copy.getJSONObject("streamSettings").getString("network"))
        assertEquals("test-only",copy.getJSONObject("streamSettings").getJSONObject("hysteriaSettings").getString("auth"))
    }

    @Test fun wireguardOutboundIsNotRewritten() {
        val out=JSONObject("""{"tag":"proxy","protocol":"wireguard","settings":{"noKernelTun":true,"secretKey":"test-only","peers":[{"endpoint":"127.0.0.1:51820"}]}}""")
        val copy=XrayConfig.build(out,42,41001,41002).getJSONArray("outbounds").getJSONObject(0)
        assertEquals(out.toString(),copy.toString())
        assertTrue(copy.getJSONObject("settings").getBoolean("noKernelTun"))
    }
}
