use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::json;
use vxsstun_profiles::{parse_import, parse_uri};

fn vmess() -> String {
    format!("vmess://{}",STANDARD.encode(json!({"v":"2","ps":"My VMess","add":"example.invalid","port":"443","id":"76c0a9a0-9a20-4a1c-b220-074e633fe383","aid":"0","net":"ws","path":"/socket","tls":"tls","scy":"auto"}).to_string()))
}
const WG:&str="[Interface]\nPrivateKey = AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=\nAddress = 10.8.0.2/32, fd00::2/128\nDNS = 1.1.1.1\n[Peer]\nPublicKey = AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI=\nEndpoint = 127.0.0.1:51820\nAllowedIPs = 0.0.0.0/0, ::/0\nPersistentKeepalive = 25";
#[test]
fn vmess_legacy_share_uri() {
    let p = parse_uri(&vmess()).unwrap();
    assert_eq!(p.protocol, "vmess");
    assert_eq!(p.name, "My VMess");
    assert_eq!(
        p.outbound.unwrap()["streamSettings"]["wsSettings"]["path"],
        "/socket"
    );
}
#[test]
fn mixed_base64_subscription() {
    let text=format!("{}\nhy2://password@example.invalid:443?sni=example.invalid\nsocks5://user:pass@example.invalid:1080\nhttps://user:pass@example.invalid:8443",vmess());
    assert_eq!(parse_import(&STANDARD.encode(text)).unwrap().len(), 4);
}
#[test]
fn http_auth_is_not_exposed() {
    let p = parse_uri("https://alice:secret@example.invalid:8443").unwrap();
    assert!(!serde_json::to_string(&p.summary())
        .unwrap()
        .contains("secret"));
    let out = p.outbound.unwrap();
    assert_eq!(out["settings"]["servers"][0]["users"][0]["pass"], "secret");
    assert_eq!(out["streamSettings"]["security"], "tls");
}
#[test]
fn socks_without_auth() {
    assert!(parse_uri("socks5://example.invalid:1080")
        .unwrap()
        .outbound
        .unwrap()["settings"]["servers"][0]["users"]
        .is_null());
}
#[test]
fn hysteria_credentials() {
    let p = parse_uri("hy2://user:pass@example.invalid").unwrap();
    assert_eq!(p.port, 443);
    assert_eq!(
        p.outbound.unwrap()["streamSettings"]["hysteriaSettings"]["auth"],
        "user:pass"
    );
}
#[test]
fn unsupported_options_not_silently_ignored() {
    for uri in [
        "hy2://secret@example.invalid?insecure=1",
        "hy2://secret@example.invalid?obfs=salamander",
        "hy2://secret@example.invalid?mport=443,8443",
        "tuic://secret@example.invalid:443",
    ] {
        assert!(parse_uri(uri).is_err());
    }
}
#[test]
fn wireguard_dual_stack() {
    let p = parse_import(WG).unwrap().remove(0);
    assert_eq!(p.protocol, "wireguard");
    let s = p.outbound.unwrap()["settings"].clone();
    assert_eq!(s["noKernelTun"], true);
    assert_eq!(s["address"].as_array().unwrap().len(), 2);
    assert_eq!(s["peers"][0]["keepAlive"], 25);
}
#[test]
fn wireguard_rejects_hooks_and_invalid_keys() {
    assert!(parse_import(&WG.replace("Address =", "PostUp = evil\nAddress =")).is_err());
    assert!(
        parse_import(&WG.replace("AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=", "short")).is_err()
    );
    assert!(parse_import(&format!("{WG}\n[Peer]\nEndpoint=127.0.0.1:50")).is_err());
}
