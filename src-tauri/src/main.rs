#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod runtime;
mod tun;
mod vault;
mod windows;
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::Manager;

#[tauri::command]
async fn dispatch(
    app: tauri::AppHandle,
    method: String,
    mut payload: Value,
) -> Result<Value, String> {
    let client = app.state::<Arc<runtime::Desktop>>().inner().clone();
    if method == "open_community" {
        windows::open_community(payload["destination"].as_str().unwrap_or_default())
            .map_err(|e| e.to_string())?;
        return Ok(json!({"opened":true}));
    }
    if method == "elevate" {
        client.allow_elevation().map_err(|e| e.to_string())?;
        windows::relaunch_elevated().map_err(|e| e.to_string())?;
        // Only after an explicit user click and successful UAC. No auto-connect.
        app.exit(0);
        return Ok(json!({"restarting":true}));
    }
    if method == "subscription" {
        let action = payload["action"].as_str().unwrap_or("add").to_owned();
        if ["remove", "rename", "addLocal"].contains(&action.as_str()) {
            return tauri::async_runtime::spawn_blocking(move || {
                client
                    .subscription(payload)
                    .and_then(|s| Ok(serde_json::to_value(s)?))
                    .map_err(|e| e.to_string())
            })
            .await
            .map_err(|_| "Ошибка подписки")?;
        }
        if action != "add" && action != "refresh" {
            return Err("Неизвестное действие подписки".into());
        }
        let raw = if action == "refresh" {
            client
                .subscription_url(payload["id"].as_str().ok_or("Нет ID подписки")?)
                .map_err(|e| e.to_string())?
        } else {
            payload["url"]
                .as_str()
                .ok_or("Укажите HTTPS-ссылку")?
                .to_owned()
        };
        let url = vxsstun_profiles::subscriptions::validate_url(&raw).map_err(|e| e.to_string())?;
        let mut response = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|_| "Ошибка HTTP-клиента")?
            .get(url)
            .header("User-Agent", concat!("VXSStun/", env!("CARGO_PKG_VERSION")))
            .send()
            .await
            .map_err(|_| "Не удалось загрузить подписку")?
            .error_for_status()
            .map_err(|_| "Сервер отклонил запрос подписки")?;
        if response.status().as_u16() != 200 {
            return Err("Подписка должна возвращать HTTP 200 без перенаправления".into());
        }
        let mut metadata = serde_json::Map::new();
        for name in vxsstun_profiles::subscription_meta::HEADERS {
            if let Some(v) = response
                .headers()
                .get(*name)
                .and_then(|v| v.to_str().ok())
                .filter(|v| v.len() <= 4096)
            {
                metadata.insert((*name).into(), json!(v));
            }
        }
        payload["headers"] = Value::Object(metadata);
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "Ошибка загрузки подписки")?
        {
            if body.len() + chunk.len() > 262144 {
                return Err("Подписка больше 256 КиБ".into());
            }
            body.extend_from_slice(&chunk);
        }
        let text = String::from_utf8(body).map_err(|_| "Подписка должна быть UTF-8")?;
        payload["body"] = json!(text);
        return tauri::async_runtime::spawn_blocking(move || {
            client
                .subscription(payload)
                .and_then(|s| Ok(serde_json::to_value(s)?))
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|_| "Ошибка импорта")?;
    }
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<Value> {
        let arg = |key: &str| {
            payload[key]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Не указан параметр {key}"))
        };
        match method.as_str() {
            "snapshot" => Ok(serde_json::to_value(client.snapshot()?)?),
            "import" => Ok(serde_json::to_value(client.import(arg("text")?)?)?),
            "profile" => Ok(serde_json::to_value(
                client.change_profile(arg("id")?, arg("action")?)?,
            )?),
            "settings" => Ok(serde_json::to_value(
                client.settings(serde_json::from_value(payload)?)?,
            )?),
            "connect" => Ok(serde_json::to_value(client.connect()?)?),
            "disconnect" => Ok(serde_json::to_value(client.disconnect()?)?),
            "latency" => Ok(json!({"ms":client.latency(arg("id")?)?})),
            "clipboard" => Ok(json!({"text":windows::clipboard_text()?})),
            _ => anyhow::bail!("Неизвестная команда"),
        }
    })
    .await
    .map_err(|_| "Операция прервана".to_string())?
    .map_err(|e| e.to_string())
}
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let resources = std::env::current_exe()?.parent().unwrap().join("runtime");
            let hashes = serde_json::from_str(include_str!("../runtime/manifest.json"))?;
            let client = runtime::Desktop::new(app.path().app_data_dir()?, resources, hashes)?;
            app.manage(client);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![dispatch])
        .build(tauri::generate_context!())
        .expect("VXSStun initialization")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                if let Some(c) = app.try_state::<Arc<runtime::Desktop>>() {
                    let _ = c.disconnect();
                }
            }
        });
}
