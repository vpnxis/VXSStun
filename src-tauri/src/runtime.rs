//! Desktop profile store and isolated connection lifecycle.
use crate::windows;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use vxsstun_profiles as profiles;
use vxsstun_profiles::{Profile, ProfileSummary};

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Telemetry {
    pub status: String,
    pub profile_id: Option<String>,
    pub error: Option<String>,
    pub connected_at: Option<u64>,
    pub bytes_up: Option<u64>,
    pub bytes_down: Option<u64>,
    pub rtt_ms: Option<u64>,
    pub carrier: Option<String>,
    pub dropped: u64,
    pub reconnects: u32,
    pub mode: String,
    pub socks_port: Option<u16>,
    pub http_port: Option<u16>,
}
pub fn edit(stats: &Arc<Mutex<Telemetry>>, f: impl FnOnce(&mut Telemetry)) {
    if let Ok(mut s) = stats.lock() {
        f(&mut s)
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: String,
    pub reconnect: bool,
    #[serde(default)]
    pub auto_update_subscriptions: bool,
    pub mode: String,
    pub selected: Option<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            reconnect: false,
            auto_update_subscriptions: false,
            mode: "proxy".into(),
            selected: None,
        }
    }
}
#[derive(Serialize, Deserialize)]
struct Store {
    profiles: Vec<Profile>,
    #[serde(default)]
    subscriptions: Vec<serde_json::Value>,
    settings: Settings,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub at: u64,
    pub kind: String,
    pub message: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub profiles: Vec<ProfileSummary>,
    pub subscriptions: Vec<serde_json::Value>,
    pub settings: Settings,
    pub connection: Telemetry,
    pub events: Vec<Event>,
    pub elevated: bool,
    pub version: String,
    pub core_version: String,
}
struct Task {
    stop: tokio::sync::watch::Sender<bool>,
    join: std::thread::JoinHandle<()>,
}
pub struct Desktop {
    store: Mutex<Store>,
    pub stats: Arc<Mutex<Telemetry>>,
    events: Mutex<Vec<Event>>,
    task: Mutex<Option<Task>>,
    pub data: PathBuf,
    pub resources: PathBuf,
    pub hashes: std::collections::HashMap<String, String>,
}
impl Desktop {
    pub fn new(
        data: PathBuf,
        resources: PathBuf,
        hashes: std::collections::HashMap<String, String>,
    ) -> Result<Arc<Self>> {
        std::fs::create_dir_all(&data)
            .context("Нет доступа к каталогу данных. Распакуйте приложение в свою папку на D:")?;
        let store = crate::vault::load(&data.join("profiles.dpapi"))?.unwrap_or_else(|| Store {
            profiles: vec![],
            subscriptions: vec![],
            settings: Settings::default(),
        });
        Ok(Arc::new(Self {
            store: Mutex::new(store),
            stats: Arc::new(Mutex::new(Telemetry {
                status: "disconnected".into(),
                ..Default::default()
            })),
            events: Mutex::new(vec![]),
            task: Mutex::new(None),
            data,
            resources,
            hashes,
        }))
    }
    fn event(&self, kind: &str, message: &str) {
        if let Ok(mut events) = self.events.lock() {
            events.push(Event {
                at: now(),
                kind: kind.into(),
                message: message.into(),
            });
            if events.len() > 200 {
                events.remove(0);
            }
        }
    }
    pub fn snapshot(&self) -> Result<Snapshot> {
        let s = self
            .store
            .lock()
            .map_err(|_| anyhow::anyhow!("Store lock"))?;
        Ok(Snapshot {
            profiles: s.profiles.iter().map(Profile::summary).collect(),
            subscriptions: profiles::subscriptions::summaries(&s.subscriptions, &s.profiles),
            settings: s.settings.clone(),
            connection: self
                .stats
                .lock()
                .map_err(|_| anyhow::anyhow!("Status lock"))?
                .clone(),
            events: self.events.lock().unwrap().clone(),
            elevated: windows::admin(),
            version: env!("CARGO_PKG_VERSION").replace('-', "."),
            core_version: "Xray 26.7.28".into(),
        })
    }
    fn persist(&self, s: &Store) -> Result<()> {
        crate::vault::save(&self.data.join("profiles.dpapi"), s)
    }
    pub fn import(&self, text: &str) -> Result<Snapshot> {
        let profiles = profiles::parse_import(text)?;
        let count = profiles.len();
        let mut s = self.store.lock().unwrap();
        ensure!(s.profiles.len() + count <= 500, "Максимум 500 профилей");
        // Commit a new store only after DPAPI + atomic persistence succeed.
        let mut next = Store {
            profiles: s.profiles.clone(),
            subscriptions: s.subscriptions.clone(),
            settings: s.settings.clone(),
        };
        let mut added = 0;
        for p in profiles {
            if !next.profiles.iter().any(|old| {
                old.subscription_id.is_none()
                    && old.outbound.is_some()
                    && old.outbound == p.outbound
            }) {
                if added == 0 {
                    next.settings.selected = Some(p.id.clone());
                }
                next.profiles.push(p);
                added += 1;
            }
        }
        self.persist(&next)?;
        *s = next;
        drop(s);
        self.event(
            "profiles",
            &format!("Добавлено профилей: {added}. Ключи сохранены через Windows DPAPI."),
        );
        self.snapshot()
    }
    pub fn change_profile(&self, id: &str, action: &str) -> Result<Snapshot> {
        let mut s = self.store.lock().unwrap();
        let mut next = Store {
            profiles: s.profiles.clone(),
            subscriptions: s.subscriptions.clone(),
            settings: s.settings.clone(),
        };
        let i = next
            .profiles
            .iter()
            .position(|p| p.id == id)
            .context("Профиль не найден")?;
        match action {
            "select" => next.settings.selected = Some(id.into()),
            "favorite" => next.profiles[i].favorite = !next.profiles[i].favorite,
            "remove" => {
                let busy = {
                    let status = self.stats.lock().unwrap();
                    status.profile_id.as_deref() == Some(id)
                        && ["connecting", "connected", "reconnecting", "disconnecting"]
                            .contains(&status.status.as_str())
                };
                ensure!(!busy, "Сначала отключите этот профиль");
                next.profiles.remove(i);
                if next.settings.selected.as_deref() == Some(id) {
                    next.settings.selected = next.profiles.first().map(|p| p.id.clone());
                }
            }
            _ => anyhow::bail!("Неизвестное действие"),
        }
        self.persist(&next)?;
        *s = next;
        drop(s);
        self.snapshot()
    }
    fn active(&self) -> bool {
        self.stats.lock().is_ok_and(|s| {
            ["connecting", "connected", "reconnecting", "disconnecting"]
                .contains(&s.status.as_str())
        })
    }
    pub fn settings(&self, settings: Settings) -> Result<Snapshot> {
        ensure!(
            ["system"].contains(&settings.theme.as_str())
                && ["tun", "proxy"].contains(&settings.mode.as_str()),
            "Некорректные настройки"
        );
        let mut s = self.store.lock().unwrap();
        ensure!(
            !self.active() || s.settings.mode == settings.mode,
            "Отключите VPN перед сменой режима"
        );
        ensure!(
            settings
                .selected
                .as_ref()
                .is_none_or(|id| s.profiles.iter().any(|p| &p.id == id)),
            "Неизвестный профиль"
        );
        let next = Store {
            profiles: s.profiles.clone(),
            subscriptions: s.subscriptions.clone(),
            settings,
        };
        self.persist(&next)?;
        *s = next;
        drop(s);
        self.snapshot()
    }
    pub fn subscription_url(&self, id: &str) -> Result<String> {
        let s = self.store.lock().unwrap();
        s.subscriptions
            .iter()
            .find(|s| s["id"] == id)
            .and_then(|s| s["url"].as_str())
            .map(str::to_owned)
            .context("Подписка не найдена")
    }
    pub fn subscription(&self, mut request: serde_json::Value) -> Result<Snapshot> {
        let mut s = self.store.lock().unwrap();
        request["activeId"] = if self.active() {
            serde_json::json!(self.stats.lock().unwrap().profile_id)
        } else {
            serde_json::Value::Null
        };
        let next: Store = serde_json::from_value(profiles::subscriptions::apply(
            serde_json::to_value(&*s)?,
            request,
            now(),
        )?)?;
        self.persist(&next)?;
        *s = next;
        drop(s);
        self.event("subscription", "Список подписок обновлён");
        self.snapshot()
    }
    pub fn latency(&self, id: &str) -> Result<u64> {
        let p = self
            .store
            .lock()
            .unwrap()
            .profiles
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .context("Профиль не найден")?;
        // TCP-connect latency only; this is explicitly not a through-tunnel speed test.
        ensure!(
            !["hysteria2", "wireguard"].contains(&p.protocol.as_str()),
            "UDP-профиль: TCP-пинг неприменим"
        );
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(async {
            tokio::time::timeout(Duration::from_secs(3), async {
                let addrs = tokio::net::lookup_host((p.host.as_str(), p.port)).await?;
                let started = Instant::now();
                for addr in addrs {
                    if tokio::net::TcpStream::connect(addr).await.is_ok() {
                        return Ok(started.elapsed().as_millis() as u64);
                    }
                }
                anyhow::bail!("TCP-проверка не получила ответ")
            })
            .await
            .context("Таймаут TCP-проверки")?
        })
    }
    pub fn connect(self: &Arc<Self>) -> Result<Snapshot> {
        let mut task = self.task.lock().unwrap();
        ensure!(!self.active(), "Подключение уже выполняется");
        if let Some(old) = task.take() {
            let _ = old.stop.send(true);
            let _ = old.join.join();
        }
        let (profile, settings) = {
            let s = self.store.lock().unwrap();
            let p = s
                .profiles
                .iter()
                .find(|p| Some(&p.id) == s.settings.selected.as_ref())
                .cloned()
                .context("Выберите сервер")?;
            (p, s.settings.clone())
        };
        ensure!(settings.mode!="tun"||windows::admin(),"Для VPN всего ПК закройте приложение и запустите его от имени администратора. Локальный Proxy работает без этих прав");
        let (tx, rx) = tokio::sync::watch::channel(false);
        let app = self.clone();
        edit(&self.stats, |s| {
            *s = Telemetry {
                status: "connecting".into(),
                profile_id: Some(profile.id.clone()),
                mode: settings.mode.clone(),
                ..Default::default()
            }
        });
        self.event(
            "connection",
            "Начато подключение. Адреса и ключи не записываются в журнал.",
        );
        let join = std::thread::spawn(move || {
            let result = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(3)
                .enable_all()
                .build()
            {
                Ok(rt) => rt.block_on(app.run(profile, settings, rx.clone())),
                Err(_) => Err(anyhow::anyhow!("Не удалось запустить сетевой исполнитель")),
            };
            let stopped = *rx.borrow();
            edit(&app.stats, |s| {
                s.status = if result.is_ok() || stopped {
                    "disconnected"
                } else {
                    "error"
                }
                .into();
                s.error = if stopped {
                    None
                } else {
                    result.err().map(|e| e.to_string())
                };
                s.connected_at = None;
                s.socks_port = None;
                s.http_port = None;
            });
            app.event(
                "connection",
                if stopped {
                    "Отключено пользователем; сетевые ресурсы освобождены."
                } else {
                    "Сетевой сеанс завершён."
                },
            );
        });
        *task = Some(Task { stop: tx, join });
        drop(task);
        self.snapshot()
    }
    pub fn allow_elevation(&self) -> Result<()> {
        ensure!(!self.active(), "Сначала отключите соединение VXSStun");
        Ok(())
    }
    async fn run(
        self: &Arc<Self>,
        profile: Profile,
        settings: Settings,
        mut stop: tokio::sync::watch::Receiver<bool>,
    ) -> Result<()> {
        let mut attempt = 0u32;
        loop {
            let result = self.xray(&profile, &settings.mode, stop.clone()).await;
            if *stop.borrow() || result.is_ok() {
                return Ok(());
            }
            if !settings.reconnect || attempt >= 3 {
                return result;
            }
            attempt += 1;
            edit(&self.stats, |s| {
                s.status = "reconnecting".into();
                s.reconnects = attempt;
                s.connected_at = None;
            });
            self.event(
                "reconnect",
                "Соединение потеряно. Ограниченная повторная попытка.",
            );
            tokio::select! {_=stop.changed()=>return Ok(()),_=tokio::time::sleep(Duration::from_secs(2u64.pow(attempt)))=>{}}
        }
    }
    async fn xray(
        &self,
        profile: &Profile,
        mode: &str,
        mut stop: tokio::sync::watch::Receiver<bool>,
    ) -> Result<()> {
        let exe = self.resources.join("xray.exe");
        windows::verified_runtime(&exe, self.hashes.get("xray.exe").context("Нет хэша Xray")?)?;
        if mode == "tun" {
            windows::verified_runtime(
                &self.resources.join("wintun.dll"),
                self.hashes.get("wintun.dll").context("Нет хэша Wintun")?,
            )?;
        }
        let a = TcpListener::bind("127.0.0.1:0")?;
        let b = TcpListener::bind("127.0.0.1:0")?;
        let m = TcpListener::bind("127.0.0.1:0")?;
        let metrics = m.local_addr()?.port();
        let socks = a.local_addr()?.port();
        let http = b.local_addr()?.port();
        let mut config = profiles::xray_config(profile, mode, socks, http)?;
        let tun_name = format!(
            "VXSStun-{}",
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        );
        if mode == "tun" {
            let interface = tokio::task::spawn_blocking(crate::tun::physical_interface).await??;
            config["inbounds"][2]["settings"]["name"] = serde_json::json!(tun_name);
            config["inbounds"][2]["settings"]["autoOutboundsInterface"] =
                serde_json::json!(interface);
            ensure!(!*stop.borrow(), "Подключение отменено");
        }
        config["metrics"] = serde_json::json!({"listen":format!("127.0.0.1:{metrics}")});
        drop(a);
        drop(b);
        drop(m);
        let mut child =
            windows::OwnedProcess::spawn(&exe, &serde_json::to_vec(&config)?, &self.resources)?;
        let mut ready = false;
        for _ in 0..40 {
            if *stop.borrow() {
                return Ok(());
            }
            ensure!(child.child.try_wait()?.is_none(),"Xray завершился при запуске. Возможно, профиль/транспорт не поддерживается этой версией ядра");
            if TcpStream::connect_timeout(
                &SocketAddr::from(([127, 0, 0, 1], http)),
                Duration::from_millis(50),
            )
            .is_ok()
            {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        ensure!(ready, "Локальный порт Xray не открылся");
        let mut check = reqwest::Client::builder().no_proxy();
        if mode == "tun" {
            let name = tun_name.clone();
            tokio::task::spawn_blocking(move || crate::tun::verify(&name)).await??;
            // Probe the OS TUN path, not the loopback HTTP inbound. Bind the TUN
            // address so a healthy direct/proxy path cannot masquerade as VPN.
            check = check.local_address(std::net::IpAddr::from([172, 29, 254, 1]));
        } else {
            check = check.proxy(reqwest::Proxy::http(format!("http://127.0.0.1:{http}"))?);
        }
        let check = check
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10))
            .build()?;
        let started = Instant::now();
        let response = tokio::select! {_=stop.changed()=>return Ok(()),r=check.get("https://www.gstatic.com/generate_204").send()=>r.map_err(|_|anyhow::anyhow!("Xray запущен, но проверка HTTPS через сервер не прошла. Соединение не подтверждено"))?};
        ensure!(
            response.status().as_u16() == 204,
            "Проверка HTTPS через сервер вернула неожиданный ответ"
        );
        edit(&self.stats, |s| {
            s.status = "connected".into();
            s.connected_at = Some(now());
            s.carrier = Some(profile.transport.to_uppercase());
            s.rtt_ms = Some(started.elapsed().as_millis() as u64);
            s.socks_port = Some(socks);
            s.http_port = Some(http);
        });
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        let mut health = tokio::time::interval_at(
            tokio::time::Instant::now() + Duration::from_secs(45),
            Duration::from_secs(45),
        );
        let mut failures = 0;
        let metrics_client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_millis(800))
            .build()?;
        loop {
            tokio::select! {
                _=stop.changed()=>return Ok(()),
                _=tick.tick()=>{
                    ensure!(child.child.try_wait()?.is_none(),"Процесс Xray остановился");
                    if let Ok(r)=metrics_client.get(format!("http://127.0.0.1:{metrics}/debug/vars")).send().await {
                        if let Ok(body)=r.bytes().await {
                            if let Ok(v)=serde_json::from_slice::<serde_json::Value>(&body) {
                                let p=&v["stats"]["outbound"]["proxy"];
                                edit(&self.stats,|s| {s.bytes_up=p["uplink"].as_u64();s.bytes_down=p["downlink"].as_u64();});
                            }
                        }
                    }
                },
                _=health.tick()=>{
                    if mode == "tun" {
                        let name=tun_name.clone();
                        tokio::task::spawn_blocking(move || crate::tun::verify(&name)).await??;
                    }
                    let response=tokio::select!{_=stop.changed()=>return Ok(()),r=check.get("https://www.gstatic.com/generate_204").send()=>r};
                    if response.is_ok_and(|r|r.status().as_u16()==204){failures=0}else{failures+=1;}
                    ensure!(failures<2,"Сервер дважды не прошёл проверку HTTPS. Канал больше не подтверждён");
                }
            }
        }
    }
    pub fn disconnect(&self) -> Result<Snapshot> {
        let mut guard = self.task.lock().unwrap();
        if let Some(task) = guard.take() {
            edit(&self.stats, |s| s.status = "disconnecting".into());
            let _ = task.stop.send(true);
            let _ = task.join.join();
        }
        edit(&self.stats, |s| {
            s.status = "disconnected".into();
            s.error = None;
        });
        drop(guard);
        self.snapshot()
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    struct Fixture {
        root: PathBuf,
        client: Arc<Desktop>,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("vxsstun-test-{}", uuid::Uuid::new_v4()));
            let client =
                Desktop::new(root.clone(), root.join("runtime"), Default::default()).unwrap();
            Self { root, client }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.client.disconnect();
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    const SAMPLE: &str = "trojan://test-only-password@example.invalid:443#Test";
    #[test]
    fn mode_switch_persists_without_starting_a_tunnel_or_admin_prompt() {
        let f = Fixture::new();
        let mut settings = f.client.snapshot().unwrap().settings;
        assert!(!settings.auto_update_subscriptions);
        settings.mode = "tun".into();
        let s = f.client.settings(settings.clone()).unwrap();
        assert_eq!(s.connection.status, "disconnected");
        assert!(f.client.task.lock().unwrap().is_none());
        let other =
            Desktop::new(f.root.clone(), f.root.join("runtime"), Default::default()).unwrap();
        assert_eq!(other.snapshot().unwrap().settings.mode, "tun");
        settings.mode = "proxy".into();
        assert_eq!(f.client.settings(settings).unwrap().settings.mode, "proxy");
    }
    #[test]
    fn legacy_settings_load_with_auto_updates_off() {
        let settings: Settings = serde_json::from_value(
            serde_json::json!({"theme":"system","mode":"proxy","reconnect":false,"selected":null}),
        )
        .unwrap();
        assert!(!settings.auto_update_subscriptions);
    }
    #[test]
    fn same_key_can_live_in_a_subscription_and_manual_configs() {
        let f = Fixture::new();
        f.client
            .subscription(serde_json::json!({"action":"addLocal","body":SAMPLE}))
            .unwrap();
        f.client.import(SAMPLE).unwrap();
        let s = f.client.import(SAMPLE).unwrap();
        assert_eq!(s.profiles.len(), 2);
        assert_eq!(
            s.profiles
                .iter()
                .filter(|p| p.subscription_id.is_none())
                .count(),
            1
        );
    }
    #[test]
    fn fresh_install_has_no_servers_or_selected_profile() {
        let f = Fixture::new();
        let s = f.client.snapshot().unwrap();
        assert!(s.profiles.is_empty());
        assert!(s.settings.selected.is_none());
        assert_eq!(s.settings.theme, "system");
        assert!(f.client.connect().is_err());
    }
    #[test]
    fn import_is_encrypted_persistent_and_deduplicated() {
        let f = Fixture::new();
        f.client.import(SAMPLE).unwrap();
        f.client.import(SAMPLE).unwrap();
        let raw = std::fs::read(f.root.join("profiles.dpapi")).unwrap();
        assert!(!String::from_utf8_lossy(&raw).contains("test-only-password"));
        let other =
            Desktop::new(f.root.clone(), f.root.join("runtime"), Default::default()).unwrap();
        let s = other.snapshot().unwrap();
        assert_eq!(s.profiles.len(), 1);
        assert!(!serde_json::to_string(&s)
            .unwrap()
            .contains("test-only-password"));
    }
    #[test]
    fn invalid_import_never_partially_changes_store() {
        let f = Fixture::new();
        assert!(f
            .client
            .import(&format!("{SAMPLE}\nnot-a-profile"))
            .is_err());
        assert!(f.client.snapshot().unwrap().profiles.is_empty());
    }
    #[test]
    fn remove_clears_selection_and_persists() {
        let f = Fixture::new();
        let s = f.client.import(SAMPLE).unwrap();
        let s = f
            .client
            .change_profile(&s.profiles[0].id, "remove")
            .unwrap();
        assert!(s.profiles.is_empty());
        assert!(s.settings.selected.is_none());
    }
    #[test]
    fn subscriptions_survive_restart_without_exposing_urls() {
        let f = Fixture::new();
        let s=f.client.subscription(serde_json::json!({"action":"add","url":"https://example.invalid/list?key=SUB-SECRET","name":"Test subscription","body":SAMPLE})).unwrap();
        let sub = s.subscriptions[0]["id"].as_str().unwrap().to_string();
        let id = s.profiles[0].id.clone();
        let other =
            Desktop::new(f.root.clone(), f.root.join("runtime"), Default::default()).unwrap();
        assert_eq!(
            other.subscription_url(&sub).unwrap(),
            "https://example.invalid/list?key=SUB-SECRET"
        );
        let next = other
            .subscription(serde_json::json!({"action":"refresh","id":sub,"body":SAMPLE}))
            .unwrap();
        assert_eq!(id, next.profiles[0].id);
        assert!(!serde_json::to_string(&next).unwrap().contains("SUB-SECRET"));
        assert!(
            !String::from_utf8_lossy(&std::fs::read(f.root.join("profiles.dpapi")).unwrap())
                .contains("SUB-SECRET")
        );
    }
    #[test]
    fn failed_subscription_does_not_overwrite_store() {
        let f = Fixture::new();
        let s = f
            .client
            .subscription(serde_json::json!({"url":"https://example.invalid/sub","body":SAMPLE}))
            .unwrap();
        let before = std::fs::read(f.root.join("profiles.dpapi")).unwrap();
        assert!(f.client.subscription(serde_json::json!({"action":"refresh","id":s.subscriptions[0]["id"],"body":"broken"})).is_err());
        assert_eq!(
            before,
            std::fs::read(f.root.join("profiles.dpapi")).unwrap()
        );
    }
}
