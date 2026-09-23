//! Explicit conversions into the pinned Xray runtime, never arbitrary executable configs.
use super::*;
use std::{collections::HashMap, net::IpAddr};

fn profile(u: &Url, protocol: &str, transport: &str, outbound: Value) -> Result<Profile> {
    let host = u
        .host_str()
        .context("Нет адреса сервера")?
        .trim_matches(['[', ']'])
        .to_owned();
    ensure!(host.len() <= 253 && !host.is_empty(), "Некорректный адрес");
    let port = u
        .port_or_known_default()
        .or(if matches!(protocol, "hysteria2") {
            Some(443)
        } else {
            None
        })
        .context("Укажите порт")?;
    ensure!(port > 0, "Некорректный порт");
    let name = unescape(u.fragment().unwrap_or(&host))?;
    ensure!(
        name.chars().count() <= 120 && !name.chars().any(char::is_control),
        "Некорректное название"
    );
    Ok(Profile {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        host,
        port,
        protocol: protocol.into(),
        transport: transport.into(),
        favorite: false,
        region: None,
        subscription_id: None,
        outbound: Some(outbound),
    })
}

pub fn proxy(u: &Url) -> Result<Profile> {
    let q: HashMap<_, _> = u.query_pairs().into_owned().collect();
    ensure!(
        !q.contains_key("insecure") || q["insecure"] == "0",
        "Проверка TLS обязательна"
    );
    ensure!(
        !q.contains_key("allowInsecure") || q["allowInsecure"] == "0",
        "Проверка TLS обязательна"
    );
    ensure!(
        u.path().is_empty() || u.path() == "/",
        "Ссылка подписки добавляется в разделе «Подписки»"
    );
    let hy = matches!(u.scheme(), "hy2" | "hysteria2");
    let socks = matches!(u.scheme(), "socks" | "socks5");
    let proto = if hy {
        "hysteria2"
    } else if socks {
        "socks5"
    } else {
        u.scheme()
    };
    let mut p = profile(u, proto, if hy { "quic" } else { "tcp" }, json!({}))?;
    let user = unescape(u.username())?;
    let pass = unescape(u.password().unwrap_or(""))?;
    ensure!(
        user.len() + pass.len() <= 2048 && !user.chars().chain(pass.chars()).any(char::is_control),
        "Некорректные данные авторизации"
    );
    let mut out = if hy {
        ensure!(!user.is_empty(), "Нет пароля Hysteria2");
        ensure!(
            !q.contains_key("obfs") && !q.contains_key("mport") && !q.contains_key("pinSHA256"),
            "Hysteria2 obfs, port hopping и pinSHA256 пока не поддерживаются"
        );
        let auth = if u.password().is_some() {
            format!("{user}:{pass}")
        } else {
            user
        };
        json!({"tag":"proxy","protocol":"hysteria","settings":{"version":2,"address":p.host,"port":p.port},"streamSettings":{"network":"hysteria","security":"tls","tlsSettings":{"serverName":q.get("sni").unwrap_or(&p.host),"allowInsecure":false,"alpn":["h3"]},"hysteriaSettings":{"version":2,"auth":auth}}})
    } else {
        let mut server = json!({"address":p.host,"port":p.port});
        if !user.is_empty() {
            server["users"] = json!([{"user":user,"pass":pass}]);
        } else {
            ensure!(u.password().is_none(), "Пароль без имени пользователя");
        }
        json!({"tag":"proxy","protocol":if socks{"socks"}else{"http"},"settings":{"servers":[server]}})
    };
    if u.scheme() == "https" {
        out["streamSettings"] = json!({"network":"tcp","security":"tls","tlsSettings":{"serverName":q.get("sni").unwrap_or(&p.host),"allowInsecure":false}});
    }
    p.outbound = Some(out);
    Ok(p)
}

pub fn vmess(s: &str) -> Result<Profile> {
    let body = s.strip_prefix("vmess://").unwrap();
    let raw = decode64(body).context("VMess: ожидается Base64 JSON")?;
    let v: Value = serde_json::from_slice(&raw).context("Некорректный VMess JSON")?;
    let field = |k: &str, d: &str| {
        v[k].as_str()
            .map(str::to_owned)
            .or_else(|| v[k].as_u64().map(|n| n.to_string()))
            .unwrap_or(d.into())
    };
    ensure!(
        field("aid", "0") == "0",
        "Устаревший VMess alterId не поддерживается"
    );
    ensure!(
        !v["allowInsecure"].as_bool().unwrap_or(false) && field("allowInsecure", "0") != "1",
        "Проверка TLS обязательна"
    );
    let id = field("id", "");
    uuid::Uuid::parse_str(&id).context("Некорректный UUID VMess")?;
    let host = field("add", "");
    let port = field("port", "")
        .parse::<u16>()
        .context("Некорректный порт VMess")?;
    let host_url = if host.contains(':') {
        format!("[{host}]")
    } else {
        host
    };
    let mut url = Url::parse(&format!("vless://{id}@{host_url}:{port}"))?;
    let net = field("net", "tcp");
    let tls = field("tls", "none");
    ensure!(
        matches!(tls.as_str(), "" | "none" | "tls"),
        "Неизвестная защита VMess"
    );
    ensure!(
        field("type", "none") == "none" || field("type", "").is_empty(),
        "VMess header type не поддерживается"
    );
    url.query_pairs_mut().extend_pairs([
        ("type", net.as_str()),
        ("security", if tls == "tls" { "tls" } else { "none" }),
        ("sni", field("sni", &field("add", "")).as_str()),
        ("host", field("host", &field("add", "")).as_str()),
        ("path", field("path", "/").as_str()),
        ("serviceName", field("path", "").as_str()),
        ("alpn", field("alpn", "h2,http/1.1").as_str()),
    ]);
    url.set_fragment(Some(&field("ps", &field("add", ""))));
    let mut p = parse_uri(url.as_str())?;
    let cipher = field("scy", "auto");
    ensure!(
        ["auto", "aes-128-gcm", "chacha20-poly1305"].contains(&cipher.as_str()),
        "Неподдерживаемый шифр VMess"
    );
    let out = p.outbound.as_mut().unwrap();
    out["protocol"] = json!("vmess");
    out["settings"] = json!({"vnext":[{"address":p.host,"port":p.port,"users":[{"id":id,"security":cipher,"alterId":0}]}]});
    p.protocol = "vmess".into();
    Ok(p)
}

pub fn wireguard(text: &str) -> Result<Profile> {
    let mut section = "";
    let mut interface = HashMap::new();
    let mut peer = HashMap::new();
    let mut peers = 0;
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            ensure!(
                line == "[Interface]" || line == "[Peer]",
                "Неизвестная секция WireGuard"
            );
            section = line;
            if line == "[Peer]" {
                peers += 1;
            }
            continue;
        }
        let (k, v) = line
            .split_once('=')
            .context("Некорректная строка WireGuard")?;
        let (k, v) = (k.trim(), v.trim());
        let target = if section == "[Interface]" {
            &mut interface
        } else {
            ensure!(section == "[Peer]", "Нет секции WireGuard");
            &mut peer
        };
        ensure!(target.insert(k, v).is_none(), "Повтор поля WireGuard");
    }
    ensure!(peers == 1, "Поддерживается ровно один WireGuard peer");
    ensure!(
        interface
            .keys()
            .all(|k| ["PrivateKey", "Address", "DNS", "MTU"].contains(k))
            && peer.keys().all(|k| [
                "PublicKey",
                "PresharedKey",
                "Endpoint",
                "AllowedIPs",
                "PersistentKeepalive"
            ]
            .contains(k)),
        "Неподдерживаемое поле WireGuard (скрипты и hooks запрещены)"
    );
    let key = |map: &HashMap<&str, &str>, k: &str| -> Result<String> {
        let v = map.get(k).context("Отсутствует ключ WireGuard")?;
        ensure!(
            decode64(v)?.len() == 32,
            "Ключ WireGuard должен быть 32 байта"
        );
        Ok((*v).into())
    };
    let secret = key(&interface, "PrivateKey")?;
    let public = key(&peer, "PublicKey")?;
    let endpoint = peer.get("Endpoint").context("Нет Endpoint WireGuard")?;
    let u = Url::parse(&format!("udp://{endpoint}"))?;
    let mut p = profile(&u, "wireguard", "udp", json!({}))?;
    ensure!(
        u.username().is_empty() && u.password().is_none() && u.query().is_none(),
        "Некорректный Endpoint"
    );
    let cidrs = |v: &str| -> Result<Vec<String>> {
        v.split(',')
            .map(|s| {
                let s = s.trim();
                let (ip, bits) = s.split_once('/').context("Ожидается CIDR")?;
                let ip: IpAddr = ip.parse().context("Некорректный IP")?;
                let bits: u8 = bits.parse().context("Некорректная маска")?;
                ensure!(
                    bits <= if ip.is_ipv4() { 32 } else { 128 },
                    "Некорректная маска"
                );
                Ok(s.into())
            })
            .collect()
    };
    let addresses = cidrs(interface.get("Address").context("Нет Address WireGuard")?)?;
    let allowed = cidrs(peer.get("AllowedIPs").unwrap_or(&"0.0.0.0/0, ::/0"))?;
    let mtu = interface.get("MTU").unwrap_or(&"1420").parse::<u16>()?;
    ensure!((1280..=1500).contains(&mtu), "MTU должен быть 1280–1500");
    let keep = peer
        .get("PersistentKeepalive")
        .unwrap_or(&"0")
        .parse::<u16>()?;
    let mut remote =
        json!({"endpoint":endpoint,"publicKey":public,"allowedIPs":allowed,"keepAlive":keep});
    if peer.contains_key("PresharedKey") {
        remote["preSharedKey"] = json!(key(&peer, "PresharedKey")?);
    }
    let mut settings = json!({"secretKey":secret,"address":addresses,"peers":[remote],"mtu":mtu,"noKernelTun":true});
    if let Some(dns) = interface.get("DNS") {
        let dns: Vec<_> = dns.split(',').map(str::trim).collect();
        ensure!(
            dns.iter().all(|s| s.parse::<IpAddr>().is_ok()),
            "DNS WireGuard должен содержать IP"
        );
        settings["remoteDNS"] = json!(dns);
    }
    p.outbound = Some(json!({"tag":"proxy","protocol":"wireguard","settings":settings}));
    Ok(p)
}
