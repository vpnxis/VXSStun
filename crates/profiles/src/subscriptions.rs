//! Same transactional subscription reconciliation on Windows and Android.
use super::*;

pub fn validate_url(value: &str) -> Result<Url> {
    ensure!(value.len() <= 8192, "Слишком длинная ссылка подписки");
    let url = Url::parse(value).context("Некорректная ссылка подписки")?;
    ensure!(
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none(),
        "Подписка требует HTTPS без логина и фрагмента"
    );
    Ok(url)
}

// request: action=add|refresh|remove; URL only for add; body contains the fetched list.
// activeId is supplied by the native backend, never accepted from WebView input.
pub fn apply(mut store: Value, request: Value, now: u64) -> Result<Value> {
    let mut profiles: Vec<Profile> = serde_json::from_value(store["profiles"].clone())?;
    let mut subs = store["subscriptions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let action = request["action"].as_str().unwrap_or("add");
    ensure!(
        ["add", "addLocal", "refresh", "remove", "rename"].contains(&action),
        "Неизвестное действие подписки"
    );
    let existing = request["id"]
        .as_str()
        .and_then(|id| subs.iter().position(|s| s["id"] == id));
    let (id, index) = if action == "add" || action == "addLocal" {
        let parsed = if action == "add" {
            Some(validate_url(
                request["url"].as_str().context("Нет ссылки подписки")?,
            )?)
        } else {
            None
        };
        ensure!(subs.len() < 32, "Максимум 32 подписки");
        ensure!(
            parsed
                .as_ref()
                .is_none_or(|url| !subs.iter().any(|s| s["url"] == url.as_str())),
            "Эта подписка уже добавлена"
        );
        let name = request["name"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(
                parsed
                    .as_ref()
                    .and_then(|url| url.host_str())
                    .unwrap_or("Моя папка"),
            )
            .trim();
        ensure!(
            name.chars().count() <= 80 && !name.chars().any(char::is_control),
            "Название подписки: до 80 символов"
        );
        let id = uuid::Uuid::new_v4().to_string();
        subs.push(json!({"id":id,"name":name,"url":parsed.as_ref().map(|url|url.as_str()),"kind":if parsed.is_some(){"remote"}else{"local"},"customName":request["name"].as_str().is_some_and(|s|!s.trim().is_empty()),"updatedAt":now}));
        (id, subs.len() - 1)
    } else {
        let i = existing.context("Подписка удалена или не найдена")?;
        (subs[i]["id"].as_str().unwrap().to_owned(), i)
    };
    if action == "rename" {
        let name = request["name"].as_str().context("Нет названия")?.trim();
        ensure!(
            !name.is_empty() && name.chars().count() <= 80 && !name.chars().any(char::is_control),
            "Название: от 1 до 80 символов"
        );
        subs[index]["name"] = json!(name);
        subs[index]["customName"] = json!(true);
        store["subscriptions"] = json!(subs);
        return Ok(store);
    }
    ensure!(
        action != "refresh" || subs[index]["url"].is_string(),
        "У локальной папки нет ссылки для обновления"
    );
    let old: Vec<_> = profiles
        .iter()
        .filter(|p| p.subscription_id.as_deref() == Some(&id))
        .cloned()
        .collect();
    let incoming = if action == "remove" {
        vec![]
    } else {
        let (body, meta) = crate::subscription_meta::parse(
            request["body"].as_str().context("Пустой ответ подписки")?,
            &request["headers"],
        )?;
        if !subs[index]["customName"].as_bool().unwrap_or(true) {
            if let Some(title) = meta["title"].as_str() {
                subs[index]["name"] = json!(title);
            }
        }
        subs[index]["metadata"] = meta;
        parse_import(&body)?
    };
    let mut replacement: Vec<Profile> = vec![];
    for mut p in incoming {
        if replacement.iter().any(|x| x.outbound == p.outbound) {
            continue;
        }
        if let Some(previous) = old.iter().find(|x| x.outbound == p.outbound) {
            p.id = previous.id.clone();
            p.favorite = previous.favorite;
        }
        p.subscription_id = Some(id.clone());
        replacement.push(p);
    }
    if let Some(active) = request["activeId"].as_str() {
        ensure!(
            !old.iter().any(|p| p.id == active) || replacement.iter().any(|p| p.id == active),
            "Сначала отключите VPN: подписка меняет активный сервер"
        );
    }
    profiles.retain(|p| p.subscription_id.as_deref() != Some(&id));
    profiles.extend(replacement);
    ensure!(profiles.len() <= 500, "Максимум 500 профилей");
    if action == "remove" {
        subs.remove(index);
    } else {
        subs[index]["updatedAt"] = json!(now);
    }
    let selected = store["settings"]["selected"].as_str();
    if selected.is_none_or(|id| !profiles.iter().any(|p| p.id == id)) {
        store["settings"]["selected"] = json!(profiles.first().map(|p| &p.id));
    }
    store["profiles"] = serde_json::to_value(profiles)?;
    store["subscriptions"] = json!(subs);
    Ok(store)
}

pub fn summaries(subscriptions: &[Value], profiles: &[Profile]) -> Vec<Value> {
    subscriptions.iter().map(|s|json!({"id":s["id"],"name":s["name"],"updatedAt":s["updatedAt"],"kind":if s["url"].is_string(){"remote"}else{"local"},"metadata":s["metadata"],"count":profiles.iter().filter(|p|p.subscription_id.as_deref()==s["id"].as_str()).count()})).collect()
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_app_vxsstun_client_NativeProfiles_updateSubscription(
    mut env: jni::JNIEnv,
    _: jni::objects::JObject,
    data: jni::objects::JString,
    request: jni::objects::JString,
) -> jni::sys::jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<String> {
        let data: String = env.get_string(&data)?.into();
        let request: String = env.get_string(&request)?.into();
        ensure!(
            data.len() <= 4194304 && request.len() <= 300000,
            "Превышен размер данных"
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        Ok(apply(
            serde_json::from_str(&data)?,
            serde_json::from_str(&request)?,
            now,
        )?
        .to_string())
    }));
    let out = match result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => json!({"error":format!("{e:#}")}).to_string(),
        Err(_) => json!({"error":"Ошибка подписки"}).to_string(),
    };
    env.new_string(out)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[cfg(test)]
mod tests {
    use super::*;
    const V: &str = "trojan://secret@example.invalid:443#one";
    fn empty() -> Value {
        json!({"profiles":[],"settings":{"selected":null}})
    }
    fn add() -> Value {
        apply(
            empty(),
            json!({"url":"https://example.invalid/sub?token=SECRET","body":V,"name":"Work"}),
            10,
        )
        .unwrap()
    }
    #[test]
    fn migration_and_no_url_in_summary() {
        let v = add();
        let profiles: Vec<Profile> = serde_json::from_value(v["profiles"].clone()).unwrap();
        let s = summaries(v["subscriptions"].as_array().unwrap(), &profiles);
        assert!(!json!(s).to_string().contains("SECRET"));
        assert_eq!(s[0]["count"], 1);
    }
    #[test]
    fn stable_ids_and_favorites() {
        let mut v = add();
        v["profiles"][0]["favorite"] = json!(true);
        let id = v["profiles"][0]["id"].clone();
        let r = json!({"action":"refresh","id":v["subscriptions"][0]["id"],"body":V,"activeId":id});
        let next = apply(v, r, 20).unwrap();
        assert_eq!(next["profiles"][0]["id"], id);
        assert_eq!(next["profiles"][0]["favorite"], true);
        assert_eq!(next["subscriptions"][0]["updatedAt"], 20);
    }
    #[test]
    fn active_profile_not_removed() {
        let v = add();
        let r = json!({"action":"remove","id":v["subscriptions"][0]["id"],"activeId":v["profiles"][0]["id"]});
        assert!(apply(v, r, 20).is_err());
    }
    #[test]
    fn bad_refresh_is_atomic() {
        let v = add();
        let r = json!({"action":"refresh","id":v["subscriptions"][0]["id"],"body":"not a config"});
        assert!(apply(v.clone(), r, 20).is_err());
        assert_eq!(v["profiles"].as_array().unwrap().len(), 1);
    }
    #[test]
    fn cannot_resurrect_removed_subscription() {
        assert!(apply(
            empty(),
            json!({"action":"refresh","id":"gone","body":V}),
            20
        )
        .is_err());
    }
    #[test]
    fn rejects_duplicate_subscription_and_http() {
        let v = add();
        assert!(apply(
            v,
            json!({"url":"https://example.invalid/sub?token=SECRET","body":V}),
            20
        )
        .is_err());
        assert!(validate_url("http://example.invalid").is_err());
        assert!(validate_url("https://u:p@example.invalid").is_err());
    }
    #[test]
    fn remove_repairs_selection() {
        let v = add();
        let r = json!({"action":"remove","id":v["subscriptions"][0]["id"]});
        let v = apply(v, r, 20).unwrap();
        assert_eq!(v["profiles"], json!([]));
        assert!(v["settings"]["selected"].is_null());
    }
    #[test]
    fn local_folder_and_rename_do_not_need_a_url() {
        let v = apply(
            empty(),
            json!({"action":"addLocal","name":"Local","body":V}),
            10,
        )
        .unwrap();
        let id = v["subscriptions"][0]["id"].clone();
        assert_eq!(v["profiles"][0]["subscriptionId"], id);
        assert!(apply(v.clone(), json!({"action":"refresh","id":id,"body":V}), 20).is_err());
        let renamed = apply(v, json!({"action":"rename","id":id,"name":"New"}), 20).unwrap();
        assert_eq!(renamed["subscriptions"][0]["name"], "New");
        assert_eq!(renamed["profiles"].as_array().unwrap().len(), 1);
    }
}
