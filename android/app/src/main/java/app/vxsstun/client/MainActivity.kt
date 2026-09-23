package app.vxsstun.client

import android.app.Activity
import android.content.ClipboardManager
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Color
import android.net.Uri
import android.net.VpnService
import android.os.Bundle
import android.webkit.*
import android.widget.FrameLayout
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.webkit.WebViewAssetLoader
import org.json.JSONObject
import java.net.InetSocketAddress
import java.net.Socket
import java.net.URL
import java.util.concurrent.Executors

class MainActivity:ComponentActivity() {
    private lateinit var web:WebView
    private lateinit var webContainer:FrameLayout
    private val work=Executors.newFixedThreadPool(3)
    // Slow DNS/TCP probes must not occupy the control/snapshot worker pool.
    private val probes=Executors.newFixedThreadPool(3)
    private var pendingVpn:Pair<String,String>?=null
    private var files:ValueCallback<Array<Uri>>?=null
    private var darkTheme=false
    override fun onCreate(savedInstanceState:Bundle?) {
        super.onCreate(savedInstanceState)
        WindowCompat.setDecorFitsSystemWindows(window,false)
        webContainer=FrameLayout(this)
        web=WebView(this)
        webContainer.addView(web,FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT,FrameLayout.LayoutParams.MATCH_PARENT))
        setContentView(webContainer);systemTheme()
        web.isVerticalScrollBarEnabled=false;web.isHorizontalScrollBarEnabled=false
        web.overScrollMode=android.view.View.OVER_SCROLL_NEVER
        web.setDefaultFocusHighlightEnabled(false)
        onBackPressedDispatcher.addCallback(this,object:OnBackPressedCallback(true){
            override fun handleOnBackPressed(){web.evaluateJavascript("window.vxsBack ? window.vxsBack() : false"){handled->if(handled!="true"&&!isDestroyed)finish()}}
        })
        ViewCompat.setOnApplyWindowInsetsListener(webContainer) {view,insets->
            // Resize the actual WebView, not its internal page padding. The same
            // viewport then contains fixed drawers, bottom sheets and 100% height.
            val types=WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout() or WindowInsetsCompat.Type.ime()
            val safe=insets.getInsets(types)
            view.setPadding(safe.left,safe.top,safe.right,safe.bottom)
            // Notify WebView of zero remaining overlap (including IME hide).
            // CONSUMED/original insets can leave stale or doubled CSS safe areas.
            WindowInsetsCompat.Builder(insets).setInsets(types,Insets.NONE).build()
        }
        ViewCompat.requestApplyInsets(webContainer)
        web.settings.apply {javaScriptEnabled=true;domStorageEnabled=false;allowFileAccess=false;allowContentAccess=false;setSupportMultipleWindows(false);mixedContentMode=WebSettings.MIXED_CONTENT_NEVER_ALLOW}
        val loader=WebViewAssetLoader.Builder().addPathHandler("/assets/",WebViewAssetLoader.AssetsPathHandler(this)).build()
        web.webViewClient=object:WebViewClient(){
            override fun shouldInterceptRequest(view:WebView,req:WebResourceRequest):WebResourceResponse? {
                val u=req.url
                if(u.scheme!="https"||u.host!="appassets.androidplatform.net")return WebResourceResponse("text/plain","UTF-8",403,"Forbidden",emptyMap(),"".byteInputStream())
                val response=loader.shouldInterceptRequest(u)
                response?.responseHeaders=mapOf("Content-Security-Policy" to "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'self'")
                return response?:WebResourceResponse("text/plain","UTF-8",404,"Not Found",emptyMap(),"".byteInputStream())
            }
            override fun shouldOverrideUrlLoading(view:WebView,req:WebResourceRequest)=true
        }
        web.webChromeClient=object:WebChromeClient(){override fun onShowFileChooser(v:WebView,callback:ValueCallback<Array<Uri>>,params:FileChooserParams):Boolean {
            files?.onReceiveValue(null);files=callback
            val images=params.acceptTypes.any{it.startsWith("image/")}
            startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).setType(if(images)"image/*" else "*/*").addCategory(Intent.CATEGORY_OPENABLE),21);return true
        }}
        web.addJavascriptInterface(Bridge(),"VXSNative")
        web.loadUrl("https://appassets.androidplatform.net/assets/web/index.html")
    }
    private fun systemTheme(){
        val dark=resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK==Configuration.UI_MODE_NIGHT_YES
        darkTheme=dark
        val background=if(dark)Color.rgb(20,25,23) else Color.rgb(245,246,242)
        web.setBackgroundColor(background);webContainer.setBackgroundColor(background)
        WindowCompat.getInsetsController(window,webContainer).apply {
            isAppearanceLightStatusBars=!dark
            isAppearanceLightNavigationBars=!dark
        }
        @Suppress("DEPRECATION")
        if(android.os.Build.VERSION.SDK_INT<35){window.statusBarColor=Color.TRANSPARENT;window.navigationBarColor=Color.TRANSPARENT}
    }
    override fun onConfigurationChanged(newConfig:Configuration){
        super.onConfigurationChanged(newConfig)
        val nextDark=newConfig.uiMode and Configuration.UI_MODE_NIGHT_MASK==Configuration.UI_MODE_NIGHT_YES
        // WebView derives prefers-color-scheme from the Activity theme. Recreate only the UI;
        // the foreground VPN service and its session remain alive.
        if(nextDark!=darkTheme)recreate() else systemTheme()
    }
    private fun reply(id:String,ok:Boolean,value:Any?){runOnUiThread{if(!isDestroyed){val json=if(value is JSONObject)value.toString() else JSONObject.quote(value?.toString()?:"");web.evaluateJavascript("window.vxsResult(${JSONObject.quote(id)},$ok,$json)",null)}}}
    inner class Bridge {
        @JavascriptInterface fun request(id:String,method:String,raw:String){
            if(id.length>24||raw.length>524288){reply(id,false,"Слишком большой запрос");return}
            (if(method=="latency")probes else work).execute {
                try {
                    val p=JSONObject(raw);val store=TunnelState.storage(this@MainActivity)
                    when(method){
                        "open_community"->{
                            val url=CommunityLinks.url(p.getString("destination"))
                            runOnUiThread{try{startActivity(Intent(Intent.ACTION_VIEW,Uri.parse(url)).addCategory(Intent.CATEGORY_BROWSABLE));reply(id,true,JSONObject().put("opened",true))}catch(t:Throwable){reply(id,false,"Не найден браузер для открытия ссылки")}};return@execute
                        }
                        "connect"->{val selected=store.selected().getString("id");runOnUiThread{try{check(pendingVpn==null&&!TunnelState.active()){ "Подключение уже выполняется" };val permission=VpnService.prepare(this@MainActivity);if(permission==null)startVpn(id,selected)else{pendingVpn=id to selected;startActivityForResult(permission,20)}}catch(t:Throwable){reply(id,false,t.message)}};return@execute}
                        "disconnect"->{startService(Intent(this@MainActivity,TunnelService::class.java).setAction("stop"))}
                        "snapshot"->{}
                        "import"->{store.add(p.getString("text"));TunnelState.event("Конфигурации импортированы и зашифрованы.")}
                        "profile"->store.change(p.getString("id"),p.getString("action"))
                        "settings"->store.updateSettings(p)
                        "subscription"->{
                            val action=p.optString("action","add")
                            require(action in listOf("add","refresh","remove","rename","addLocal")){"Неизвестное действие подписки"}
                            if(action=="add"||action=="refresh"){
                                val url=if(action=="refresh")store.subscriptionUrl(p.getString("id"))else p.getString("url")
                                val body=try{subscription(url)}catch(t:Throwable){error("Не удалось загрузить HTTPS-подписку. Проверьте ссылку и сеть.")}
                                p.put("body",body.first).put("headers",body.second)
                            }
                            store.subscription(p);TunnelState.event("Список подписок обновлён.")
                        }
                        "latency"->{val profile=store.profile(p.getString("id"));require(profile.getString("protocol") !in listOf("hysteria2","wireguard")){"UDP-профиль: TCP-пинг неприменим"};val start=System.nanoTime();Socket().use{it.connect(InetSocketAddress(profile.getString("host"),profile.getInt("port")),3000)};reply(id,true,JSONObject().put("ms",(System.nanoTime()-start)/1000000));return@execute}
                        "clipboard"->{runOnUiThread {val clipboard=getSystemService(ClipboardManager::class.java);reply(id,true,JSONObject().put("text",clipboard.primaryClip?.getItemAt(0)?.coerceToText(this@MainActivity)?.toString()?.take(262144)?:""))};return@execute}
                        else->error("Неизвестная команда")
                    }
                    reply(id,true,TunnelState.snapshot(this@MainActivity))
                }catch(t:Throwable){reply(id,false,t.message?.take(240)?:"Операция не выполнена")}
            }
        }
    }
    private fun startVpn(id:String,profile:String){startForegroundService(Intent(this,TunnelService::class.java).setAction("connect").putExtra("profileId",profile));reply(id,true,TunnelState.snapshot(this))}
    private fun subscription(url:String):Pair<String,JSONObject> {
        require(url.length<=8192){"Слишком длинная ссылка"}
        val u=URL(url);require(u.protocol=="https"&&u.userInfo==null&&u.ref==null){"Подписки загружаются только по HTTPS"}
        val c=u.openConnection() as javax.net.ssl.HttpsURLConnection
        try{
            c.connectTimeout=8000;c.readTimeout=8000;c.instanceFollowRedirects=false
            c.setRequestProperty("User-Agent","VXSStun/${BuildConfig.VERSION_NAME}")
            require(c.responseCode==200){"Сервер отклонил запрос подписки"}
            val output=java.io.ByteArrayOutputStream()
            val deadline=android.os.SystemClock.elapsedRealtime()+15000
            c.inputStream.use {stream->val buffer=ByteArray(8192);while(true){val n=stream.read(buffer);if(n<0)break;require(output.size()+n<=262144){"Подписка больше 256 КиБ"};check(android.os.SystemClock.elapsedRealtime()<deadline){"Таймаут подписки"};output.write(buffer,0,n)}}
            val headers=JSONObject()
            for(name in listOf("profile-title","subscription-userinfo","announce","profile-update-interval")) {
                c.getHeaderField(name)?.takeIf{it.length<=4096}?.let{headers.put(name,it)}
            }
            return output.toString("UTF-8") to headers
        }finally{c.disconnect()}
    }
    @Deprecated("Activity result compatibility")
    override fun onActivityResult(requestCode:Int,resultCode:Int,data:Intent?){super.onActivityResult(requestCode,resultCode,data)
        if(requestCode==20){val p=pendingVpn;pendingVpn=null;if(p!=null){if(resultCode==Activity.RESULT_OK)startVpn(p.first,p.second)else reply(p.first,false,"Разрешение на VPN не предоставлено")}}
        if(requestCode==21){files?.onReceiveValue(if(resultCode==Activity.RESULT_OK&&data?.data!=null)arrayOf(data.data!!)else null);files=null}
    }
    override fun onDestroy(){web.removeJavascriptInterface("VXSNative");web.destroy();work.shutdownNow();probes.shutdownNow();files?.onReceiveValue(null);super.onDestroy()}
}
