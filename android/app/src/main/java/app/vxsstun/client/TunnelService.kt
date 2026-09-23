package app.vxsstun.client

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.net.VpnService
import android.os.ParcelFileDescriptor
import libXray.DialerController
import libXray.LibXray
import org.json.JSONArray
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.InetSocketAddress
import java.net.Proxy
import java.net.URL
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicLong

class TunnelService:VpnService(),DialerController {
    private val worker=Executors.newSingleThreadExecutor()
    private val revision=AtomicLong()
    private var tun:ParcelFileDescriptor?=null
    @Volatile private var request:HttpURLConnection?=null
    private var metrics=0
    private var health=0
    override fun protectFd(fd:Long):Boolean=protect(fd.toInt())
    override fun onStartCommand(intent:Intent?,flags:Int,startId:Int):Int {
        if(intent?.action=="stop") {stopTunnel();return START_NOT_STICKY}
        if(intent?.action!="connect"||TunnelState.active())return START_NOT_STICKY
        val id=intent.getStringExtra("profileId")?:return START_NOT_STICKY
        val epoch=revision.incrementAndGet()
        TunnelState.status="connecting";TunnelState.profileId=id;TunnelState.error=null;TunnelState.connectedAt=null;TunnelState.up=null;TunnelState.down=null;TunnelState.reconnects=0
        foreground("Подключение…")
        worker.execute {
            try {
                val profile=TunnelState.storage(this).profile(id)
                var attempts=0
                while(revision.get()==epoch) {
                    try {
                        startEngine(profile,epoch)
                        TunnelState.status="connected";TunnelState.connectedAt=System.currentTimeMillis()/1000
                        TunnelState.event("VPN подключён; проверка HTTPS пройдена.")
                        foreground("VPN подключён")
                        var ticks=0;var failures=0
                        while(revision.get()==epoch) {
                            Thread.sleep(1000);checkEpoch(epoch)
                            check(api("getXrayState").optJSONObject("data")?.optBoolean("running")==true){"Ядро остановлено"}
                            pollTraffic()
                            if(++ticks%45==0){if(verify())failures=0 else failures++;check(failures<2){"Сервер не отвечает через туннель"}}
                        }
                    } catch(t:Throwable) {
                        release()
                        if(revision.get()!=epoch)break
                        if(!TunnelState.storage(this).settings().optBoolean("reconnect")||attempts>=3)throw t
                        attempts++;TunnelState.reconnects=attempts;TunnelState.status="reconnecting";TunnelState.connectedAt=null
                        TunnelState.event("Восстановление соединения: попытка $attempts из 3.")
                        repeat((1 shl attempts)*4){Thread.sleep(250);checkEpoch(epoch)}
                    }
                }
            } catch(t:Throwable) {
                if(revision.get()==epoch){TunnelState.status="error";TunnelState.error=t.message?.takeIf {it.length<220}?:"Не удалось запустить VPN";TunnelState.event("Подключение завершилось ошибкой.")}
            } finally {
                release();TunnelState.connectedAt=null
                if(TunnelState.status!="error")TunnelState.status="disconnected"
                stopForeground(STOP_FOREGROUND_REMOVE);stopSelf()
            }
        }
        return START_NOT_STICKY
    }
    private fun checkEpoch(epoch:Long){check(revision.get()==epoch){"Подключение отменено"}}
    private fun startEngine(profile:JSONObject,epoch:Long) {
        release();checkEpoch(epoch)
        // Include IPv6 so it cannot silently bypass an IPv4-only VPN. The engine decides routing.
        tun=Builder().setSession("VXSStun").setMtu(1400).addAddress("172.28.0.1",30).addAddress("fd73:7678::1",126)
            .addRoute("0.0.0.0",0).addRoute("::",0).addDnsServer("1.1.1.1").setBlocking(false).establish()?:error("Android не создал VPN-интерфейс")
        val ports=api("getFreePorts",JSONObject().put("count",2)).getJSONObject("data").getJSONArray("ports")
        metrics=ports.getInt(0);health=ports.getInt(1)
        SocketGuard.claim(this);LibXray.setDNS(SocketGuard,"1.1.1.1:53")
        val config=XrayConfig.build(profile.getJSONObject("outbound"),tun!!.fd,metrics,health)
        check(JSONObject(LibXray.invoke(XrayConfig.startRequest(config))).optBoolean("success")){"Ядро отклонило конфигурацию"}
        var running=false
        for(i in 0 until 40){checkEpoch(epoch);if(api("getXrayState").optJSONObject("data")?.optBoolean("running")==true){running=true;break};Thread.sleep(100)}
        check(running){"Ядро не запустилось"};checkEpoch(epoch)
        check(verify()){ "Ядро запущено, но проверка HTTPS через сервер не прошла" };checkEpoch(epoch)
    }
    private fun api(method:String,payload:JSONObject=JSONObject())=JSONObject(LibXray.invoke(XrayConfig.request(method,payload)))
    private fun verify():Boolean=runCatching {
        val c=URL("https://www.gstatic.com/generate_204").openConnection(Proxy(Proxy.Type.HTTP,InetSocketAddress("127.0.0.1",health))) as HttpURLConnection
        request=c
        try{c.connectTimeout=4000;c.readTimeout=4000;c.instanceFollowRedirects=false;c.useCaches=false;c.responseCode==204}finally{c.disconnect();request=null}
    }.getOrDefault(false)
    private fun pollTraffic(){runCatching {
        val c=URL("http://127.0.0.1:$metrics/debug/vars").openConnection(Proxy.NO_PROXY) as HttpURLConnection
        try{c.connectTimeout=400;c.readTimeout=500;val p=JSONObject(c.inputStream.bufferedReader().use {it.readText()}).optJSONObject("stats")?.optJSONObject("outbound")?.optJSONObject("proxy");if(p!=null){TunnelState.up=p.optLong("uplink").coerceAtLeast(0);TunnelState.down=p.optLong("downlink").coerceAtLeast(0)}}finally{c.disconnect()}
    }}
    private fun release(){SocketGuard.release(this){runCatching{api("stopXray")};runCatching{LibXray.resetDNS()}};runCatching{tun?.close()};tun=null}
    private fun stopTunnel(){revision.incrementAndGet();request?.disconnect();if(TunnelState.active())TunnelState.status="disconnecting";else{stopForeground(STOP_FOREGROUND_REMOVE);stopSelf()};TunnelState.event("Отключение запрошено пользователем.")}
    override fun onRevoke(){stopTunnel();super.onRevoke()}
    override fun onDestroy(){revision.incrementAndGet();request?.disconnect();worker.shutdown();super.onDestroy()}
    private fun foreground(text:String) {
        val nm=getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel("vpn","VPN",NotificationManager.IMPORTANCE_LOW))
        val content=PendingIntent.getActivity(this,0,Intent(this,MainActivity::class.java),PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val stop=PendingIntent.getService(this,1,Intent(this,TunnelService::class.java).setAction("stop"),PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val n=Notification.Builder(this,"vpn").setSmallIcon(R.drawable.ic_launcher).setContentTitle("VXSStun").setContentText(text).setContentIntent(content).setOngoing(true).addAction(Notification.Action.Builder(null,"Отключить",stop).build()).build()
        startForeground(1,n)
    }
}
