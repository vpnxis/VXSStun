use anyhow::{bail, ensure, Context, Result};
use base64::{
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE_NO_PAD},
    Engine,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use url::Url;
mod protocols;
pub mod subscription_meta;
pub mod subscriptions;

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_app_vxsstun_client_NativeProfiles_parseImport(
    mut env: jni::JNIEnv,
    _: jni::objects::JObject,
    text: jni::objects::JString,
) -> jni::sys::jstring {
    // Never unwind across JNI; all parse errors are JSON, not exception text with secrets.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<String> {
        let text: String = env.get_string(&text)?.into();
        Ok(serde_json::to_string(&parse_import(&text)?)?)
    }));
    let response = match result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => json!({"error":format!("{e:#}")}).to_string(),
        Err(_) => json!({"error":"Ошибка обработки конфигурации"}).to_string(),
    };
    env.new_string(response)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub transport: String,
    pub favorite: bool,
    pub region: Option<String>,
    #[serde(default)]
    pub subscription_id: Option<String>,
    // Never returned to the WebView in snapshots or diagnostic exports.
    pub outbound: Option<Value>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub transport: String,
    pub favorite: bool,
    pub region: Option<String>,
    pub subscription_id: Option<String>,
}
impl Profile {
    pub fn summary(&self) -> ProfileSummary {
        ProfileSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            protocol: self.protocol.clone(),
            host: self.host.clone(),
            port: self.port,
            transport: self.transport.clone(),
            favorite: self.favorite,
            region: self.region.clone(),
            subscription_id: self.subscription_id.clone(),
        }
    }
}
fn decode64(s: &str) -> Result<Vec<u8>> {
    STANDARD
        .decode(s)
        .or_else(|_| STANDARD_NO_PAD.decode(s))
        .or_else(|_| URL_SAFE_NO_PAD.decode(s))
        .context("Некорректный Base64")
}
fn unescape(s: &str) -> Result<String> {
    // Use form decoding but preserve the literal '+' from URL userinfo/fragments.
    Ok(
        url::form_urlencoded::parse(format!("v={}", s.replace('+', "%2B")).as_bytes())
            .next()
            .context("Пустое поле")?
            .1
            .into_owned(),
    )
}
pub fn parse_import(text: &str) -> Result<Vec<Profile>> {
    ensure!(text.len() <= 256 * 1024, "Импорт ограничен 256 КиБ");
    let text = text.trim().trim_start_matches('\u{feff}').trim();
    if text.starts_with("[Interface]") {
        return Ok(vec![protocols::wireguard(text)?]);
    }
    let decoded;
    let source = if !text.contains("://") {
        // Providers commonly wrap Base64 lists at 76 columns. Strip only ASCII
        // whitespace here; URI lines themselves remain strictly validated.
        let compact: String = text.chars().filter(|c| !c.is_ascii_whitespace()).collect();
        decoded = String::from_utf8(decode64(&compact)?)
            .context("Ожидаются ссылки или Base64-подписка")?;
        decoded.as_str()
    } else {
        text
    };
    let lines: Vec<_> = source
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    ensure!(
        !lines.is_empty() && lines.len() <= 100,
        "За один импорт — от 1 до 100 серверов"
    );
    // Transactional: reject the whole input, never silently skip broken profiles.
    lines
        .into_iter()
        .enumerate()
        .map(|(i, l)| parse_uri(l).with_context(|| format!("Строка {}", i + 1)))
        .collect()
}
pub fn parse_uri(s: &str) -> Result<Profile> {
    ensure!(
        s.len() <= 8192 && !s.chars().any(|c| c.is_control()),
        "Некорректная длина ссылки"
    );
    let u = Url::parse(s).context("Некорректная ссылка сервера")?;
    if s.starts_with("vmess://") {
        return protocols::vmess(s);
    }
    if ["socks", "socks5", "http", "https", "hysteria2", "hy2"].contains(&u.scheme()) {
        return protocols::proxy(&u);
    }
    let proto = u.scheme();
    ensure!(
        ["vless", "trojan", "ss"].contains(&proto),
        "Неизвестный протокол. Доступны VLESS, VMess, Trojan, Shadowsocks, Hysteria2, WireGuard, SOCKS5 и HTTP(S)"
    );
    let host = u
        .host_str()
        .context("Нет адреса сервера")?
        .trim_matches(['[', ']'])
        .to_string();
    ensure!(!host.is_empty() && host.len() <= 253, "Некорректный адрес");
    let port = u.port().context("Укажите порт сервера")?;
    ensure!(port > 0, "Порт должен быть от 1 до 65535");
    let q: std::collections::HashMap<String, String> = u.query_pairs().into_owned().collect();
    let get = |k: &str, default: &str| q.get(k).cloned().unwrap_or_else(|| default.into());
    ensure!(
        !q.contains_key("allowInsecure") || get("allowInsecure", "") == "0",
        "Отключение проверки сертификата не поддерживается"
    );
    ensure!(
        !q.contains_key("plugin"),
        "Shadowsocks plugins пока не поддерживаются"
    );
    let name = unescape(u.fragment().unwrap_or(""))?;
    let name = if name.trim().is_empty() {
        host.clone()
    } else {
        name
    };
    ensure!(
        name.chars().count() <= 120 && !name.chars().any(|c| c.is_control()),
        "Название слишком длинное"
    );
    let transport = get("type", "tcp");
    ensure!(
        ["tcp", "raw", "ws", "grpc", "xhttp", "httpupgrade"].contains(&transport.as_str()),
        "Этот транспорт пока не поддерживается"
    );
    let secret = unescape(u.username())?;
    ensure!(
        !secret.is_empty() && secret.len() <= 1024,
        "В ссылке нет ключа"
    );
    let security = get("security", if proto == "trojan" { "tls" } else { "none" });
    ensure!(
        ["none", "tls", "reality"].contains(&security.as_str()),
        "Неизвестная защита транспорта"
    );
    ensure!(proto != "trojan" || security == "tls", "Trojan требует TLS");
    let mut stream = json!({"network":transport,"security":security});
    let sni = get("sni", &get("serverName", &host));
    if security == "tls" {
        stream["tlsSettings"] =
            json!({"serverName":sni,"allowInsecure":false,"fingerprint":get("fp","chrome")});
        if let Some(alpn) = q.get("alpn") {
            stream["tlsSettings"]["alpn"] = json!(alpn.split(',').collect::<Vec<_>>());
        }
    } else if security == "reality" {
        ensure!(proto == "vless", "REALITY поддерживается только с VLESS");
        let key = get("pbk", "");
        ensure!(
            URL_SAFE_NO_PAD.decode(&key).is_ok_and(|b| b.len() == 32),
            "Неверный public key REALITY"
        );
        let sid = get("sid", "");
        ensure!(
            sid.len() <= 16 && sid.len() % 2 == 0 && sid.bytes().all(|b| b.is_ascii_hexdigit()),
            "Неверный short ID REALITY"
        );
        stream["realitySettings"] = json!({"serverName":sni,"fingerprint":get("fp","chrome"),"publicKey":key,"shortId":sid});
    }
    match transport.as_str() {
        "ws" => {
            stream["wsSettings"] =
                json!({"path":get("path","/"),"headers":{"Host":get("host",&host)}})
        }
        "grpc" => {
            stream["grpcSettings"] =
                json!({"serviceName":get("serviceName",""),"multiMode":get("mode","")=="multi"})
        }
        "xhttp" => {
            let mode = get("mode", "auto");
            ensure!(
                ["auto", "packet-up", "stream-up", "stream-one"].contains(&mode.as_str()),
                "Неизвестный режим XHTTP"
            );
            stream["xhttpSettings"] =
                json!({"path":get("path","/"),"host":get("host",&host),"mode":mode});
        }
        "httpupgrade" => {
            stream["httpupgradeSettings"] = json!({"path":get("path","/"),"host":get("host",&host)})
        }
        _ => {}
    }
    let settings = match proto {
        "vless" => {
            uuid::Uuid::parse_str(&secret).context("Некорректный UUID VLESS")?;
            ensure!(
                get("encryption", "none") == "none",
                "Этот тип VLESS encryption пока не поддерживается"
            );
            let flow = get("flow", "");
            ensure!(
                ["", "xtls-rprx-vision"].contains(&flow.as_str()),
                "Неизвестный VLESS flow"
            );
            json!({"vnext":[{"address":host,"port":port,"users":[{"id":secret,"encryption":"none","flow":flow}]}]})
        }
        "trojan" => json!({"servers":[{"address":host,"port":port,"password":secret}]}),
        "ss" => {
            ensure!(
                security == "none" && transport == "tcp",
                "Для SS используйте стандартную SIP002-ссылку"
            );
            let credentials = if let Some(pass) = u.password() {
                format!("{secret}:{}", unescape(pass)?)
            } else {
                String::from_utf8(decode64(&secret)?).context("Некорректный ключ SS")?
            };
            let (method, password) = credentials
                .split_once(':')
                .context("В SS отсутствуют метод и пароль")?;
            ensure!(
                [
                    "aes-128-gcm",
                    "aes-256-gcm",
                    "chacha20-ietf-poly1305",
                    "2022-blake3-aes-128-gcm",
                    "2022-blake3-aes-256-gcm",
                    "2022-blake3-chacha20-poly1305"
                ]
                .contains(&method),
                "Не поддерживается метод Shadowsocks"
            );
            ensure!(!password.is_empty(), "Пустой пароль SS");
            json!({"servers":[{"address":host,"port":port,"method":method,"password":password}]})
        }
        _ => bail!("Неизвестный протокол"),
    };
    Ok(Profile {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        protocol: proto.into(),
        host,
        port,
        transport,
        favorite: false,
        region: None,
        subscription_id: None,
        outbound: Some(
            json!({"tag":"proxy","protocol":if proto=="ss"{"shadowsocks"}else{proto},"settings":settings,"streamSettings":stream}),
        ),
    })
}
pub fn xray_config(profile: &Profile, mode: &str, socks: u16, http: u16) -> Result<Value> {
    ensure!(["proxy", "tun"].contains(&mode), "Неизвестный режим");
    let out = profile
        .outbound
        .clone()
        .context("Этот профиль не относится к Xray")?;
    let mut inbounds = vec![
        json!({"tag":"socks","listen":"127.0.0.1","port":socks,"protocol":"socks","settings":{"auth":"noauth","udp":true}}),
        json!({"tag":"http","listen":"127.0.0.1","port":http,"protocol":"http","settings":{}}),
    ];
    if mode == "tun" {
        inbounds.push(json!({"tag":"tun","protocol":"tun","settings":{"name":"VXSStun","desc":"VXSStun","mtu":1500,"gateway":["172.29.254.1/30","fd58:7665:6e6f::1/126"],"dns":["1.1.1.1","2606:4700:4700::1111"],"autoSystemRoutingTable":["0.0.0.0/0","::/0"],"autoOutboundsInterface":"auto"}}));
    }
    Ok(
        json!({"log":{"loglevel":"none"},"inbounds":inbounds,"outbounds":[out],"stats":{},"policy":{"system":{"statsOutboundUplink":true,"statsOutboundDownlink":true}}}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const V:&str="vless://76c0a9a0-9a20-4a1c-b220-074e633fe383@example.com:443?security=tls&type=ws&path=%2Fsocket#My%20server";
    #[test]
    fn parses_vless() {
        let p = parse_uri(V).unwrap();
        assert_eq!(p.name, "My server");
        assert_eq!(
            p.outbound.unwrap()["streamSettings"]["wsSettings"]["path"],
            "/socket"
        );
    }
    #[test]
    fn rejects_insecure() {
        assert!(parse_uri(&format!("{}&allowInsecure=1", V.split('#').next().unwrap())).is_err());
    }
    #[test]
    fn import_atomic() {
        assert!(parse_import(&format!("{V}\nnot-a-server")).is_err());
    }
    #[test]
    fn import_base64() {
        assert_eq!(parse_import(&STANDARD.encode(V)).unwrap().len(), 1);
    }
    #[test]
    fn imports_wrapped_base64_and_bom_without_relaxing_uri_validation() {
        let encoded = STANDARD.encode(format!("{V}\n{V}"));
        let wrapped = encoded
            .as_bytes()
            .chunks(76)
            .map(|c| std::str::from_utf8(c).unwrap())
            .collect::<Vec<_>>()
            .join("\r\n");
        assert_eq!(
            parse_import(&format!("\u{feff}  {wrapped}\n"))
                .unwrap()
                .len(),
            2
        );
        assert!(parse_import(&format!("{V}\ncorrupted")).is_err());
    }
    #[test]
    fn tls_trojan() {
        let p = parse_uri("trojan://a%3Ab@example.com:443#T").unwrap();
        assert_eq!(
            p.outbound.unwrap()["settings"]["servers"][0]["password"],
            "a:b"
        );
    }
    #[test]
    fn safe_summary() {
        let p = parse_uri(V).unwrap();
        assert!(!serde_json::to_string(&p.summary())
            .unwrap()
            .contains("76c0a9a0"));
    }
    #[test]
    fn no_external_listener() {
        let c = xray_config(&parse_uri(V).unwrap(), "proxy", 2080, 2081).unwrap();
        for i in c["inbounds"].as_array().unwrap() {
            assert_eq!(i["listen"], "127.0.0.1");
        }
    }
    #[test]
    fn tun_no_direct_bypass() {
        let c = xray_config(&parse_uri(V).unwrap(), "tun", 2080, 2081).unwrap();
        assert_eq!(c["outbounds"].as_array().unwrap().len(), 1);
        assert_eq!(
            c["inbounds"][2]["settings"]["autoOutboundsInterface"],
            "auto"
        );
    }
    #[test]
    fn rejects_unsupported() {
        assert!(parse_uri("https://example.com/subscription").is_err());
        assert!(parse_uri("vless://bad@example.com:443").is_err());
    }
    #[test]
    fn ss_sip002() {
        let p = parse_uri(&format!(
            "ss://{}@example.com:8388#SS",
            URL_SAFE_NO_PAD.encode("aes-256-gcm:secret")
        ))
        .unwrap();
        assert_eq!(p.protocol, "ss");
    }
}
