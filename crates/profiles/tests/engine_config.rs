use base64::{engine::general_purpose::STANDARD, Engine};
use std::{
    io::Write,
    process::{Command, Stdio},
};
use vxsstun_profiles::{parse_uri, xray_config};

// Only parses configuration with -test. Never starts TUN or modifies routes.
#[test]
#[ignore = "Set VXSSTUN_XRAY to the verified runtime, then run --ignored"]
fn generated_configs_are_accepted_by_xray() {
    let exe = std::env::var("VXSSTUN_XRAY").expect("runtime path");
    let output = Command::new(&exe).arg("version").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("Xray 26.7.28 "), "Pinned runtime must include Windows auto-route support; old Xray silently ignores those fields");
    let links=[
        "vless://76c0a9a0-9a20-4a1c-b220-074e633fe383@example.invalid:443?security=tls&type=tcp",
        "vless://76c0a9a0-9a20-4a1c-b220-074e633fe383@example.invalid:443?security=tls&type=ws&path=%2Fsocket",
        "vless://76c0a9a0-9a20-4a1c-b220-074e633fe383@example.invalid:443?security=tls&type=grpc&serviceName=test",
        "vless://76c0a9a0-9a20-4a1c-b220-074e633fe383@example.invalid:443?security=tls&type=xhttp&mode=auto",
        "vless://76c0a9a0-9a20-4a1c-b220-074e633fe383@example.invalid:443?security=tls&type=httpupgrade",
        "trojan://test-password@example.invalid:443",
        "ss://aes-256-gcm:test-password@example.invalid:8388",
        "socks5://user:pass@example.invalid:1080",
        "http://user:pass@example.invalid:8080",
        "https://user:pass@example.invalid:8443",
        "hy2://test-password@example.invalid:443?sni=example.invalid",
    ];
    let mut profiles = links
        .iter()
        .map(|s| parse_uri(s).unwrap())
        .collect::<Vec<_>>();
    let vmess = serde_json::json!({"v":"2","add":"example.invalid","port":443,"id":"76c0a9a0-9a20-4a1c-b220-074e633fe383","net":"ws","tls":"tls","path":"/socket"});
    profiles.push(parse_uri(&format!("vmess://{}", STANDARD.encode(vmess.to_string()))).unwrap());
    profiles.extend(vxsstun_profiles::parse_import("[Interface]\nPrivateKey=AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=\nAddress=10.8.0.2/32\n[Peer]\nPublicKey=AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI=\nEndpoint=127.0.0.1:51820\nAllowedIPs=0.0.0.0/0,::/0").unwrap());
    for profile in profiles {
        for mode in ["proxy", "tun"] {
            let config = xray_config(&profile, mode, 41001, 41002).unwrap();
            let mut child = Command::new(&exe)
                .args(["run", "-test", "-config", "stdin:"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(config.to_string().as_bytes())
                .unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
