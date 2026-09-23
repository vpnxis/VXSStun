//! Display-only subscription metadata. Never applies remote routing/settings.
use super::*;
pub const HEADERS: &[&str] = &[
    "profile-title",
    "subscription-userinfo",
    "announce",
    "profile-update-interval",
];

fn text(value: &str, limit: usize) -> Option<String> {
    let decoded = if let Some(encoded) = value.strip_prefix("base64:") {
        String::from_utf8(decode64(encoded).ok()?).ok()?
    } else {
        value.to_owned()
    };
    let cleaned: String = decoded
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect();
    (!cleaned.is_empty()).then_some(cleaned)
}

pub fn parse(body: &str, headers: &Value) -> Result<(String, Value)> {
    ensure!(body.len() <= 262144, "Подписка больше 256 КиБ");
    let decoded;
    let source = if !body.contains("://") && !body.trim_start().starts_with("[Interface]") {
        let compact: String = body.chars().filter(|c| !c.is_ascii_whitespace()).collect();
        decoded = String::from_utf8(decode64(&compact)?).context("Подписка должна быть UTF-8")?;
        decoded.as_str()
    } else {
        body
    };
    let mut raw = serde_json::Map::new();
    let mut lines = vec![];
    for line in source.trim_start_matches('\u{feff}').lines().map(str::trim) {
        if let Some(comment) = line.strip_prefix('#') {
            if let Some((key, value)) = comment.split_once(':') {
                let key = key.trim().to_ascii_lowercase();
                if HEADERS.contains(&key.as_str()) && value.len() <= 4096 {
                    raw.insert(key, json!(value.trim()));
                }
            }
        } else if !line.is_empty() {
            lines.push(line);
        }
    }
    for key in HEADERS {
        if let Some(v) = headers[*key].as_str().filter(|v| v.len() <= 4096) {
            raw.insert((*key).into(), json!(v));
        }
    }
    let raw = Value::Object(raw);
    let mut meta = json!({});
    for (key, field, limit) in [
        ("profile-title", "title", 80),
        ("announce", "announce", 200),
    ] {
        if let Some(value) = raw[key].as_str().and_then(|v| text(v, limit)) {
            meta[field] = json!(value);
        }
    }
    if let Some(hours) = raw["profile-update-interval"]
        .as_str()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| (1..=168).contains(v))
    {
        meta["updateIntervalHours"] = json!(hours);
    }
    if let Some(info) = raw["subscription-userinfo"].as_str() {
        for pair in info.split(';') {
            if let Some((key, value)) = pair.trim().split_once('=') {
                let key = key.trim();
                if ["upload", "download", "total", "expire"].contains(&key) {
                    if let Ok(n) = value.trim().parse::<u64>() {
                        let max = if key == "expire" {
                            253402300799
                        } else {
                            9007199254740991
                        };
                        if n <= max {
                            meta[key] = json!(n);
                        }
                    }
                }
            }
        }
    }
    Ok((lines.join("\n"), meta))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn headers_win_and_remote_commands_are_not_applied() {
        let (body, meta) = parse("#profile-title: Body\n#routing-enable: 0\n#announce: <script>x</script>\ntrojan://secret@example.invalid:443", &json!({"profile-title":"base64:VGVzdA==","subscription-userinfo":"upload=20; download=30; total=100; expire=1900000000"})).unwrap();
        assert_eq!(meta["title"], "Test");
        assert_eq!(meta["upload"], 20);
        assert!(meta.get("routing-enable").is_none());
        assert!(body.starts_with("trojan://"));
        assert_eq!(meta["announce"], "<script>x</script>"); // rendered as React text, not HTML
    }
    #[test]
    fn base64_body_and_malformed_optional_values() {
        let body=STANDARD.encode("#profile-title: Name\n#subscription-userinfo: total=-1; expire=999999999999; upload=wat\n#profile-update-interval: 0\ntrojan://s@example.invalid:443");
        let (_, m) = parse(&body, &Value::Null).unwrap();
        assert_eq!(m, json!({"title":"Name"}));
    }
}
