package app.vxsstun.client

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.AtomicFile
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import org.json.JSONArray
import org.json.JSONObject

class ProfileStore(context: Context) {
    private val file = AtomicFile(java.io.File(context.noBackupFilesDir, "profiles.aes"))
    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey("vxsstun-profiles-v1", null) as? SecretKey)?.let { return it }
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES,"AndroidKeyStore").apply {
            init(KeyGenParameterSpec.Builder("vxsstun-profiles-v1",KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT).setBlockModes(KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE).build())
        }.generateKey()
    }
    private var data = load()
    private fun load(): JSONObject {
        if(!file.baseFile.exists()) return JSONObject().put("profiles",JSONArray()).put("settings",JSONObject().put("theme","system").put("mode","tun").put("reconnect",false).put("selected",JSONObject.NULL))
        require(file.baseFile.length() in 29..4194304) { "Повреждено хранилище профилей" }
        val raw=file.readFully()
        val cipher=Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE,key(),GCMParameterSpec(128,raw.copyOfRange(0,12)))
        return JSONObject(String(cipher.doFinal(raw.copyOfRange(12,raw.size)),Charsets.UTF_8))
    }
    private fun save(next:JSONObject) {
        val cipher=Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE,key())
        val raw=cipher.iv+cipher.doFinal(next.toString().toByteArray(Charsets.UTF_8))
        require(raw.size<=4194304) { "Хранилище больше 4 МиБ" }
        val out=file.startWrite()
        try {out.write(raw);file.finishWrite(out)} catch(t:Throwable) {file.failWrite(out);throw t}
        data=next
    }
    @Synchronized fun summaries(): JSONArray = JSONArray().also { arr ->
        val profiles=data.getJSONArray("profiles")
        for(i in 0 until profiles.length()) arr.put(JSONObject(profiles.getJSONObject(i).toString()).apply {remove("outbound")})
    }
    @Synchronized fun settings(): JSONObject = JSONObject(data.getJSONObject("settings").toString())
    @Synchronized fun subscriptions(): JSONArray = JSONArray().also { result ->
        val list=data.optJSONArray("subscriptions")?:JSONArray()
        val profiles=data.getJSONArray("profiles")
        for(i in 0 until list.length()) {
            val s=list.getJSONObject(i)
            val count=(0 until profiles.length()).count { profiles.getJSONObject(it).optString("subscriptionId")==s.getString("id") }
            result.put(JSONObject().put("id",s.getString("id")).put("name",s.getString("name")).put("updatedAt",s.optLong("updatedAt")).put("count",count).put("kind",if(s.isNull("url"))"local" else "remote").put("metadata",s.optJSONObject("metadata")?:JSONObject()))
        }
    }
    @Synchronized fun subscriptionUrl(id:String):String {
        val list=data.optJSONArray("subscriptions")?:JSONArray()
        for(i in 0 until list.length()) if(list.getJSONObject(i).getString("id")==id)return list.getJSONObject(i).getString("url")
        error("Подписка не найдена")
    }
    @Synchronized fun subscription(request:JSONObject) {
        request.put("activeId",if(TunnelState.active())TunnelState.profileId else JSONObject.NULL)
        val next=JSONObject(NativeProfiles.updateSubscription(data.toString(),request.toString()))
        require(!next.has("error")){next.optString("error")}
        save(next)
    }
    @Synchronized fun selected(): JSONObject {
        val id=settings().optString("selected")
        return profile(id)
    }
    @Synchronized fun profile(id:String):JSONObject {
        val list=data.getJSONArray("profiles")
        for(i in 0 until list.length()) if(list.getJSONObject(i).getString("id")==id) return JSONObject(list.getJSONObject(i).toString())
        error("Выберите сервер")
    }
    @Synchronized fun add(text:String) {
        require(text.length<=262144) {"Импорт больше 256 КиБ"}
        val parsed=NativeProfiles.parseImport(text)
        if(parsed.startsWith("{")) error(JSONObject(parsed).optString("error","Ошибка импорта"))
        val incoming=JSONArray(parsed)
        val next=JSONObject(data.toString()); val list=next.getJSONArray("profiles")
        require(list.length()+incoming.length()<=500) {"Максимум 500 профилей"}
        var first:String?=null
        for(i in 0 until incoming.length()) {
            val p=incoming.getJSONObject(i)
            val duplicate=(0 until list.length()).any {list.getJSONObject(it).isNull("subscriptionId") && list.getJSONObject(it).getJSONObject("outbound").toString()==p.getJSONObject("outbound").toString()}
            if(!duplicate) {list.put(p);if(first==null)first=p.getString("id")}
        }
        if(first!=null)next.getJSONObject("settings").put("selected",first)
        save(next)
    }
    @Synchronized fun change(id:String,action:String) {
        val next=JSONObject(data.toString());val list=next.getJSONArray("profiles")
        val index=(0 until list.length()).firstOrNull {list.getJSONObject(it).getString("id")==id}?:error("Профиль не найден")
        when(action) {
            "select"->next.getJSONObject("settings").put("selected",id)
            "favorite"->list.getJSONObject(index).put("favorite",!list.getJSONObject(index).optBoolean("favorite"))
            "remove"->{require(!TunnelState.active()||TunnelState.profileId!=id){"Сначала отключите сервер"};list.remove(index);if(next.getJSONObject("settings").optString("selected")==id)next.getJSONObject("settings").put("selected",if(list.length()>0)list.getJSONObject(0).getString("id") else JSONObject.NULL)}
            else->error("Неизвестное действие")
        }
        save(next)
    }
    @Synchronized fun updateSettings(settings:JSONObject) {
        require(settings.optString("theme")=="system"&&settings.optString("mode")=="tun") {"Некорректные настройки"}
        if(!settings.isNull("selected"))profile(settings.getString("selected"))
        save(JSONObject(data.toString()).put("settings",settings))
    }
}
