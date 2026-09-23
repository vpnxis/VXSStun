import React, { useState, useEffect, useLayoutEffect, useRef } from "react";
import { createRoot } from "react-dom/client";
import { call, platform, emptySnapshot } from "./bridge";
import {
  bytes,
  elapsed,
  isActive,
  importKind,
  powerStatusLabel,
} from "./format";
import logo from "./assets/icon.svg";
import "./style.css";
import ProfileLibrary from "./ProfileLibrary";
import { filterProfiles, dueSubscriptions } from "./library";
import { readQrFile } from "./qr";

const paths = {
  qr: "M3 3h6v6H3zM15 3h6v6h-6zM3 15h6v6H3zM15 15h3v3h3v3h-6zM21 12h-3M12 3v3M12 12h3v3M12 18v3",
  plus: "M12 5v14M5 12h14",
  link: "M10 13a5 5 0 0 0 7 .5l3-3a5 5 0 0 0-7-7l-1.7 1.7M14 11a5 5 0 0 0-7-.5l-3 3a5 5 0 0 0 7 7l1.7-1.7",
  folder:
    "M3 7V5a2 2 0 0 1 2-2h5l2 3h7a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z",
  edit: "m15 4 5 5M4 20l4-1L21 6l-4-4L4 15v5Z",
  refresh:
    "M20 7v5h-5M4 17v-5h5M5 8a8 8 0 0 1 13-3l2 2M4 17l2 2a8 8 0 0 0 13-3",
  globe:
    "M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0ZM3 12h18M12 3c5 5 5 13 0 18-5-5-5-13 0-18Z",
  settings:
    "m9 3-.7 2.2-2 .9L4 5.7 2.5 8l1.5 1.8-.2 2.3L2 14l1.2 2.6 2.5.1 1.7 1.6.4 2.4H11l1.2-2 2.3-.5 2.1 1.1 2.2-2-1-2.2.5-2.4 2-1V8.5l-2.4-.7-1.3-1.9.3-2.4-2.8-1.1-1.7 1.6L9 3ZM15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z",
  stats: "M4 4h16v16H4zM7 14l3-4 3 5 4-7",
  logs: "M7 3 3 7l4 4M17 3l4 4-4 4M11 3l2 8M4 14v6h16v-6",
  info: "M12 11v6M12 7h.01M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z",
  power: "M12 2v10M6 5a9 9 0 1 0 12 0",
  search: "M16 16l5 5M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0Z",
  ping: "M4 18a9 9 0 1 1 16 0M12 13l5-6M12 13h.01",
  more: "M4 12h.01M12 12h.01M20 12h.01",
  chevron: "m9 5 7 7-7 7",
  down: "m5 9 7 7 7-7",
  close: "m5 5 14 14M19 5 5 19",
  menu: "M4 6h16M4 12h16M4 18h16",
  trash: "M3 6h18M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7M14 10v7",
  check: "m5 12 4 4L20 5",
  copy: "M8 8h13v13H8zM16 8V3H3v13h5",
  upload: "M12 16V3m-5 5 5-5 5 5M4 16v5h16v-5",
  arrow: "m14 5-7 7 7 7",
  star: "m12 3 2.8 5.7 6.2.9-4.5 4.4 1.1 6.2-5.6-3-5.6 3 1.1-6.2L3 9.6l6.2-.9Z",
  external: "M14 3h7v7M21 3 10 14M10 3H3v18h18v-7",
  shield: "M12 3 4 6v6c0 5 8 9 8 9s8-4 8-9V6ZM8 12l3 3 5-6",
};
function Icon({ name, size = 22, ...props }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      {...props}
    >
      <path d={paths[name] || paths.globe} />
    </svg>
  );
}
const labels = {
  disconnected: "Отключён",
  connecting: "Подключение",
  connected: "Подключён",
  reconnecting: "Восстановление",
  disconnecting: "Отключение",
  error: "Ошибка подключения",
};
const nav = [
  ["servers", "globe", "Серверы"],
  ["subscriptions", "link", "Подписки"],
  ["settings", "settings", "Настройки"],
  ["stats", "stats", "Статистика"],
  ["logs", "logs", "Логи"],
];
function App() {
  const [s, setS] = useState(emptySnapshot),
    [tab, setTab] = useState("servers"),
    [drawer, setDrawer] = useState(false),
    [modal, setModal] = useState(null),
    [input, setInput] = useState(""),
    [subName, setSubName] = useState(""),
    [group, setGroup] = useState("all"),
    [query, setQuery] = useState(""),
    [busy, setBusy] = useState(false),
    [notice, setNotice] = useState(""),
    [pings, setPings] = useState({}),
    [pingBusy, setPingBusy] = useState(false),
    [powerRequest, setPowerRequest] = useState(null),
    [sort, setSort] = useState("original"),
    [protocol, setProtocol] = useState("all"),
    [batchProgress, setBatchProgress] = useState(null),
    [clock, setClock] = useState(Date.now()),
    [menu, setMenu] = useState(null);
  const active = isActive(s.connection.status),
    selected = s.profiles.find((p) => p.id === s.settings.selected),
    connected = s.profiles.find((p) => p.id === s.connection.profileId),
    display = active ? connected || selected : selected;
  const mutations = useRef(0);
  const operation = useRef(false);
  const workspace = useRef(null);
  const [favorites, setFavorites] = useState(false);
  const pingStop = useRef(false);
  const qrInput = useRef(null);
  const [qrBusy, setQrBusy] = useState(false);
  const refreshAttempts = useRef({});
  const refreshSchedule = JSON.stringify(s.subscriptions);
  useEffect(() => {
    if (!s.settings.autoUpdateSubscriptions) return;
    let cancelled = false;
    async function refreshDue() {
      if (
        cancelled ||
        document.visibilityState !== "visible" ||
        operation.current
      )
        return;
      const sub = dueSubscriptions(
        JSON.parse(refreshSchedule),
        Date.now(),
        refreshAttempts.current,
      )[0];
      if (!sub) return;
      refreshAttempts.current[sub.id] = Date.now();
      try {
        await act("subscription", { action: "refresh", id: sub.id });
      } catch {
        /* act displays the error; retry is bounded by dueSubscriptions. */
      }
    }
    const timer = setInterval(refreshDue, 60000);
    refreshDue();
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [s.settings.autoUpdateSubscriptions, refreshSchedule]);
  useLayoutEffect(() => {
    if (workspace.current) {
      workspace.current.scrollTop = 0;
      workspace.current.querySelector(".content").scrollTop = 0;
    }
    setMenu(null);
  }, [tab]);
  useEffect(() => {
    if (!menu) return;
    const dismiss = (e) => {
      if (!e.target.closest(".popover,.rowmenu,.folder-actions")) setMenu(null);
    };
    document.addEventListener("pointerdown", dismiss);
    return () => document.removeEventListener("pointerdown", dismiss);
  }, [menu]);
  useEffect(() => {
    let alive = true,
      inflight = false;
    async function poll() {
      if (inflight) return;
      if (operation.current) return;
      inflight = true;
      const revision = mutations.current;
      try {
        const v = await call("snapshot");
        if (alive && revision === mutations.current && !operation.current)
          setS(v);
      } catch (e) {
        if (alive) setNotice(e.message);
      } finally {
        inflight = false;
      }
    }
    poll();
    const timer = setInterval(poll, 1000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, []);
  useEffect(() => {
    const t = setInterval(() => setClock(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);
  useEffect(() => {
    window.vxsBack = () => {
      if (modal) {
        if (!operation.current) setModal(null);
        return true;
      }
      if (drawer) {
        setDrawer(false);
        return true;
      }
      if (menu) {
        setMenu(null);
        return true;
      }
      if (tab !== "servers") {
        setTab("servers");
        return true;
      }
      return false;
    };
    return () => {
      delete window.vxsBack;
    };
  }, [modal, drawer, menu, tab]);
  useEffect(() => {
    if (!notice) return;
    const t = setTimeout(() => setNotice(""), 6500);
    return () => clearTimeout(t);
  }, [notice]);
  useEffect(() => {
    const f = (e) => {
      if (e.key === "Escape") {
        if (!operation.current) setModal(null);
        setMenu(null);
        setDrawer(false);
      }
    };
    window.addEventListener("keydown", f);
    return () => window.removeEventListener("keydown", f);
  }, []);
  useEffect(() => {
    if (!modal) return;
    const previous = document.activeElement;
    const focusFrame = requestAnimationFrame(() => {
      const dialog = document.querySelector(".modal");
      if (!dialog?.contains(document.activeElement))
        dialog
          ?.querySelector("button:not(:disabled)")
          ?.focus({ preventScroll: true });
    });
    const trap = (e) => {
      if (e.key !== "Tab") return;
      const items = [
        ...document.querySelectorAll(
          ".modal button:not(:disabled), .modal textarea, .modal input:not([hidden]), .modal [role='button'][tabindex='0']",
        ),
      ];
      if (!items.length) return;
      const first = items[0],
        last = items[items.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", trap);
    return () => {
      cancelAnimationFrame(focusFrame);
      window.removeEventListener("keydown", trap);
      previous?.focus?.();
    };
  }, [modal]);
  async function act(method, payload = {}) {
    if (operation.current) throw new Error("Дождитесь завершения операции");
    operation.current = true;
    mutations.current++;
    setBusy(true);
    try {
      const v = await call(method, payload);
      if (v.profiles) setS(v);
      return v;
    } catch (e) {
      setNotice(e.message);
      throw e;
    } finally {
      mutations.current++;
      operation.current = false;
      setBusy(false);
    }
  }
  const safe = (fn) => () => {
    Promise.resolve()
      .then(fn)
      .catch((e) => setNotice(e.message || "Операция не выполнена"));
  };
  function openImport() {
    setInput("");
    setSubName("");
    setModal("add");
    setDrawer(false);
  }
  function openSubscription() {
    setInput("");
    setSubName("");
    setModal("subscription");
    setDrawer(false);
  }
  async function pasteImport(keepFolder = false) {
    const { text = "" } = await call("clipboard");
    if (!text.trim()) throw new Error("Буфер обмена пуст");
    setInput(text);
    setSubName("");
    setModal(
      keepFolder || importKind(text) === "subscription"
        ? "subscription"
        : "import",
    );
  }
  async function importNow() {
    const value = input.trim();
    if (!value) return;
    const url =
      modal === "subscription" || importKind(value) === "subscription";
    const remote = importKind(value) === "subscription";
    await act(
      url ? "subscription" : "import",
      url
        ? remote
          ? { action: "add", url: value, name: subName }
          : { action: "addLocal", body: value, name: subName }
        : { text: value },
    );
    setInput("");
    setModal(null);
    setNotice(
      url
        ? remote
          ? "Подписка сохранена. Серверы загружены."
          : "Локальная папка сохранена"
        : "Серверы добавлены",
    );
  }
  async function ping(id) {
    if (!id || pingBusy) return;
    setPingBusy(true);
    try {
      const v = await call("latency", { id });
      setPings((p) => ({ ...p, [id]: v.ms }));
    } catch {
      setPings((p) => ({ ...p, [id]: "Недоступен" }));
    } finally {
      setPingBusy(false);
    }
  }
  async function pingAll(targets = profiles) {
    if (pingBusy) return;
    const queue = targets.filter(
      (p) => !["wireguard", "hysteria2"].includes(p.protocol),
    );
    pingStop.current = false;
    setPingBusy(true);
    setBatchProgress({ done: 0, total: queue.length });
    let index = 0,
      done = 0;
    try {
      await Promise.all(
        Array.from({ length: Math.min(3, queue.length) }, async () => {
          while (!pingStop.current && index < queue.length) {
            const p = queue[index++];
            try {
              const v = await call("latency", { id: p.id });
              setPings((old) => ({ ...old, [p.id]: v.ms }));
            } catch {
              setPings((old) => ({ ...old, [p.id]: "Недоступен" }));
            }
            done++;
            setBatchProgress({ done, total: queue.length });
          }
        }),
      );
    } finally {
      setPingBusy(false);
      setBatchProgress(null);
    }
  }
  async function refreshSubscription(sub) {
    await act("subscription", { action: "refresh", id: sub.id });
    setPings((old) =>
      Object.fromEntries(
        Object.entries(old).filter(
          ([id]) =>
            !s.profiles.some((p) => p.id === id && p.subscriptionId === sub.id),
        ),
      ),
    );
    setNotice("Подписка обновлена");
  }
  async function refreshAll() {
    if (operation.current) return;
    let success = 0,
      failed = 0;
    for (const sub of subscriptions.filter((x) => x.kind !== "local")) {
      try {
        await act("subscription", { action: "refresh", id: sub.id });
        success++;
      } catch {
        failed++;
      }
    }
    setNotice(
      `Обновлено: ${success}. Ошибок: ${failed}. Старые данные неудачных обновлений сохранены.`,
    );
  }
  const subscriptions = s.subscriptions || [];
  const profiles = filterProfiles(s.profiles, {
    query,
    favorites,
    protocol,
    sort,
    pings,
  });
  function modePicker(inSettings = false) {
    if (platform === "android")
      return inSettings ? <span className="tag">VPN (TUN)</span> : null;
    return (
      <div className="mode-picker">
        <div
          className="mode-buttons"
          role="group"
          aria-label="Режим подключения"
        >
          {[
            ["tun", "VPN (TUN)"],
            ["proxy", "Прокси"],
          ].map(([value, label]) => (
            <button
              key={value}
              aria-pressed={s.settings.mode === value}
              disabled={busy || active}
              onClick={safe(() =>
                act("settings", { ...s.settings, mode: value }),
              )}
            >
              {label}
            </button>
          ))}
        </div>
        {active && <small>Сначала отключите текущее соединение VXSStun.</small>}
      </div>
    );
  }
  const filters = (
    <div className="library-filters">
      <button
        className={`favoritefilter ${favorites ? "chosen" : ""}`}
        aria-pressed={favorites}
        onClick={() => setFavorites(!favorites)}
      >
        <Icon name="star" size={17} />
        Избранное
      </button>
      <select
        aria-label="Сортировка"
        value={sort}
        onChange={(e) => setSort(e.target.value)}
      >
        <option value="original">Порядок провайдера</option>
        <option value="name">По имени</option>
        <option value="ping">По TCP-пингу</option>
        <option value="protocol">По протоколу</option>
      </select>
      <select
        aria-label="Протокол"
        value={protocol}
        onChange={(e) => setProtocol(e.target.value)}
      >
        <option value="all">Все протоколы</option>
        {[...new Set(s.profiles.map((p) => p.protocol))].sort().map((p) => (
          <option key={p} value={p}>
            {p.toUpperCase()}
          </option>
        ))}
      </select>
      {(query || favorites || protocol !== "all" || group !== "all") && (
        <button
          className="textbutton"
          onClick={() => {
            setQuery("");
            setFavorites(false);
            setProtocol("all");
            setGroup("all");
          }}
        >
          Сбросить фильтры
        </button>
      )}
      {batchProgress && (
        <div className="ping-progress" role="status">
          Проверено {batchProgress.done} / {batchProgress.total}
          <button
            className="textbutton"
            onClick={() => {
              pingStop.current = true;
            }}
          >
            Остановить
          </button>
        </div>
      )}
    </div>
  );
  const library = (onlySubscriptions = false) => (
    <ProfileLibrary
      Icon={Icon}
      profiles={profiles}
      allProfiles={s.profiles}
      subscriptions={subscriptions}
      group={onlySubscriptions ? "all" : group}
      onlySubscriptions={onlySubscriptions}
      selected={s.settings.selected}
      connected={active ? s.connection.profileId : null}
      pings={pings}
      busy={busy}
      pingBusy={pingBusy}
      menu={menu}
      setMenu={setMenu}
      onSelect={(p) =>
        safe(() => act("profile", { id: p.id, action: "select" }))()
      }
      onFavorite={(p) =>
        safe(() => act("profile", { id: p.id, action: "favorite" }))()
      }
      onPing={ping}
      onPingGroup={pingAll}
      onRefresh={(sub) => safe(() => refreshSubscription(sub))()}
      onRename={(sub) => {
        setSubName(sub.name);
        setModal({ renameSubscription: sub });
      }}
      onDetails={(p) => setModal({ details: p })}
      onRemove={(p) => setModal({ remove: p })}
      onRemoveSubscription={(sub) => setModal({ removeSubscription: sub })}
      onAdd={openImport}
    />
  );
  return (
    <div className="app" data-platform={platform}>
      <input
        ref={qrInput}
        type="file"
        hidden
        accept="image/png,image/jpeg,image/webp"
        onChange={async (e) => {
          const file = e.target.files?.[0];
          e.target.value = "";
          if (!file) return;
          setQrBusy(true);
          try {
            const value = await readQrFile(file);
            setInput(value);
            setSubName("");
            setModal(
              importKind(value) === "subscription" ? "subscription" : "import",
            );
            setNotice(
              "QR-код прочитан локально. Проверьте данные перед добавлением.",
            );
          } catch (error) {
            setNotice(error.message || "Не удалось прочитать QR-код");
          } finally {
            setQrBusy(false);
          }
        }}
      />
      <header className="mobilebar" inert={!!modal || drawer}>
        <button
          className="iconbutton"
          aria-label="Меню"
          onClick={() => setDrawer(true)}
        >
          <Icon name="menu" />
        </button>
        <span className="mobilebrand">
          <img src={logo} alt="" />
          <span>
            <strong>VXSStun</strong>
            <small>by VenoXiss</small>
          </span>
        </span>
        <button
          className="iconbutton"
          aria-label="Добавить сервер"
          onClick={openImport}
        >
          <Icon name="plus" />
        </button>
      </header>
      {drawer && <div className="scrim" onClick={() => setDrawer(false)} />}
      <aside className={`sidebar ${drawer ? "open" : ""}`} inert={!!modal}>
        <div className="brand">
          <img className="brandmark" src={logo} alt="" />
          <span>
            VXSStun<small>ЛИЧНЫЙ VPN-КЛИЕНТ</small>
          </span>
          <button
            className="iconbutton drawerclose"
            aria-label="Закрыть меню"
            onClick={() => setDrawer(false)}
          >
            <Icon name="close" />
          </button>
        </div>
        <button className="navitem addnav" onClick={openImport}>
          <Icon name="plus" />
          <span>Добавить</span>
        </button>
        <nav>
          {nav.map(([id, icon, label]) => (
            <button
              key={id}
              className={`navitem ${tab === id ? "current" : ""}`}
              aria-current={tab === id ? "page" : undefined}
              onClick={() => {
                setTab(id);
                setDrawer(false);
              }}
            >
              <Icon name={icon} />
              <span>{label}</span>
            </button>
          ))}
        </nav>
        <div className="teamnav">
          <small>VenoXiss / vpnxis</small>
          <button
            onClick={safe(() =>
              call("open_community", { destination: "telegram" }),
            )}
          >
            Telegram <Icon name="external" size={13} />
          </button>
          <button
            onClick={safe(() =>
              call("open_community", { destination: "github" }),
            )}
          >
            GitHub <Icon name="external" size={13} />
          </button>
        </div>
        <button
          className={`navitem aboutnav ${tab === "about" ? "current" : ""}`}
          onClick={() => {
            setTab("about");
            setDrawer(false);
          }}
        >
          <Icon name="info" />
          <span>О программе</span>
        </button>
      </aside>
      <main
        ref={workspace}
        className={`workspace ${tab === "servers" ? "server-page" : ""} ${s.profiles.length ? "has-profiles" : "is-empty"}`}
        inert={!!modal || drawer}
      >
        <section className="content">
          {s.preview && (
            <p className="quietnote">Демо-интерфейс · VPN не запускается</p>
          )}
          <header className="pageheader">
            <small className="eyebrow">РАБОЧЕЕ ПРОСТРАНСТВО</small>
            <h1>{nav.find((n) => n[0] === tab)?.[2] || "О программе"}</h1>
          </header>
          {tab === "servers" && (
            <>
              <div className="searchrow">
                <label className="search">
                  <input
                    aria-label="Поиск серверов"
                    placeholder="Введите текст для поиска"
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                  />
                  <Icon name="search" />
                </label>
                <button
                  className="iconbutton"
                  aria-label="Проверить TCP-пинг всех серверов"
                  disabled={pingBusy || !s.profiles.length}
                  onClick={() => pingAll()}
                >
                  <Icon name="ping" />
                </button>
                <button
                  className="iconbutton"
                  aria-label="Добавить сервер"
                  onClick={openImport}
                >
                  <Icon name="plus" />
                </button>
              </div>
              {!s.profiles.length ? (
                <div className="empty">
                  <div className="emptyicon">
                    <Icon name="globe" size={42} />
                  </div>
                  <h2>Добавьте первый сервер</h2>
                  <p>
                    Вставьте ссылку подключения или HTTPS-ссылку на подписку от
                    вашего провайдера.
                  </p>
                  <div className="emptyactions">
                    <button className="primary" onClick={safe(pasteImport)}>
                      <Icon name="copy" size={19} />
                      Из буфера
                    </button>
                    <button className="secondary" onClick={openSubscription}>
                      <Icon name="link" size={19} />
                      Подписка
                    </button>
                  </div>
                  <small>Встроенных серверов и регистрации нет</small>
                  <button className="textbutton" onClick={openImport}>
                    Другие способы <Icon name="chevron" size={16} />
                  </button>
                </div>
              ) : (
                <>
                  {filters}
                  {library()}
                </>
              )}
              {!!s.profiles.length && (
                <p className="quietnote">
                  Пинг — время TCP-подключения к серверу, не скорость VPN.
                </p>
              )}
            </>
          )}
          {tab === "subscriptions" && (
            <div className="subscriptionpage">
              <p className="subheading">
                Подписки и локальные папки. Каждый список отдельно от одиночных
                конфигураций.
              </p>
              <div className="subscription-toolbar">
                <button className="primary" onClick={openSubscription}>
                  <Icon name="plus" size={18} />
                  Добавить подписку
                </button>
                <button
                  className="secondary"
                  disabled={
                    busy || !subscriptions.some((x) => x.kind !== "local")
                  }
                  onClick={safe(refreshAll)}
                >
                  <Icon name="refresh" size={18} />
                  Обновить все
                </button>
              </div>
              {subscriptions.length ? (
                <>
                  <label className="search subscription-search">
                    <input
                      aria-label="Поиск в подписках"
                      placeholder="Поиск в папках"
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                    />
                    <Icon name="search" />
                  </label>
                  {filters}
                  {library(true)}
                </>
              ) : (
                <div className="empty small-empty">
                  <Icon name="folder" size={36} />
                  <h2>Каждой подписке — своя папка</h2>
                  <p>
                    HTTPS-ссылка создаст обновляемую подписку. Список ключей —
                    локальную папку без фоновых запросов.
                  </p>
                </div>
              )}
            </div>
          )}
          {tab === "settings" && (
            <div className="settingslist">
              <h2>Подключение</h2>
              <div className="setting">
                <div>
                  <strong>Режим работы</strong>
                  <p>
                    {platform === "android"
                      ? "VPN для всего устройства через Android VpnService."
                      : "TUN требует запуска EXE от администратора. Локальный прокси не меняет настройки Windows."}
                  </p>
                </div>
                {modePicker(true)}
              </div>
              <div className="setting">
                <div>
                  <strong>Повторное подключение</strong>
                  <p>До трёх попыток после потери соединения.</p>
                </div>
                <input
                  aria-label="Автопереподключение"
                  type="checkbox"
                  className="switch"
                  checked={s.settings.reconnect}
                  onChange={(e) =>
                    safe(() =>
                      act("settings", {
                        ...s.settings,
                        reconnect: e.target.checked,
                      }),
                    )()
                  }
                />
              </div>
              <h2>Подписки</h2>
              <div className="setting">
                <div>
                  <strong>Обновлять при открытом приложении</strong>
                  <p>
                    Только HTTPS-подписки: по интервалу провайдера, иначе раз в
                    сутки. По умолчанию выключено.
                  </p>
                </div>
                <input
                  aria-label="Автообновление подписок"
                  type="checkbox"
                  className="switch"
                  disabled={busy}
                  checked={!!s.settings.autoUpdateSubscriptions}
                  onChange={(e) => {
                    const value = e.target.checked;
                    safe(() =>
                      act("settings", {
                        ...s.settings,
                        autoUpdateSubscriptions: value,
                      }),
                    )();
                  }}
                />
              </div>
              <h2>Оформление</h2>
              <div className="setting">
                <div>
                  <strong>Тема устройства</strong>
                  <p>Светлая и тёмная тема переключаются автоматически.</p>
                </div>
                <span className="badge">Системная</span>
              </div>
              <h2>Приватность</h2>
              <div className="infoblock">
                Конфигурации хранятся на устройстве: Windows DPAPI или Android
                Keystore. Нет аккаунта, аналитики и встроенных VPN-серверов.
                Проверка соединения обращается к публичной HTTPS-странице Google
                через выбранный туннель.
              </div>
              <div className="infoblock safety-note">
                <Icon name="shield" size={20} />
                <span>
                  Собственного kill switch пока нет. При обрыве туннеля возможен
                  прямой выход в сеть. На Android можно включить системную
                  блокировку соединений без VPN.
                </span>
              </div>
              <button className="settingslink" onClick={() => setTab("logs")}>
                <Icon name="logs" />
                Журнал событий
                <Icon name="chevron" size={18} />
              </button>
              <button className="settingslink" onClick={() => setTab("about")}>
                <Icon name="info" />О VXSStun и команда
                <Icon name="chevron" size={18} />
              </button>
            </div>
          )}
          {tab === "stats" && (
            <>
              <p className="subheading">Текущий сеанс</p>
              <div className="statgrid">
                <article>
                  <small>Получено</small>
                  <strong>{bytes(s.connection.bytesDown)}</strong>
                </article>
                <article>
                  <small>Отправлено</small>
                  <strong>{bytes(s.connection.bytesUp)}</strong>
                </article>
                <article>
                  <small>Время подключения</small>
                  <strong>
                    {elapsed(active ? s.connection.connectedAt : null, clock)}
                  </strong>
                </article>
                <article>
                  <small>Переподключения</small>
                  <strong>{s.connection.reconnects || 0}</strong>
                </article>
              </div>
              <p className="quietnote">
                Счётчики ядра обновляются во время подключения. Отсутствующие
                данные отмечены «—».
              </p>
            </>
          )}
          {tab === "logs" && (
            <>
              <p className="subheading">
                События текущего запуска · без ключей и содержимого трафика
              </p>
              <div className="loglist">
                {s.events.length ? (
                  s.events
                    .slice()
                    .reverse()
                    .map((e, i) => (
                      <div className="logrow" key={i}>
                        <time>
                          {new Date(e.at * 1000).toLocaleTimeString("ru")}
                        </time>
                        <span>{e.message}</span>
                      </div>
                    ))
                ) : (
                  <p className="nomatch">Событий пока нет</p>
                )}
              </div>
            </>
          )}
          {tab === "about" && (
            <div className="about">
              <img
                className="brandmark large"
                src={logo}
                alt="Логотип VXSStun"
              />
              <h2>VXSStun</h2>
              <p>
                Открытый клиент команды VenoXiss / vpnxis. Ваши серверы, ваши
                подписки.
              </p>
              <div className="communitylinks">
                <button
                  className="secondary"
                  onClick={safe(() =>
                    call("open_community", { destination: "telegram" }),
                  )}
                >
                  Telegram · @VenoXiss <Icon name="external" size={16} />
                </button>
                <button
                  className="secondary"
                  onClick={safe(() =>
                    call("open_community", { destination: "github" }),
                  )}
                >
                  GitHub · vpnxis <Icon name="external" size={16} />
                </button>
              </div>
              <dl>
                <div>
                  <dt>Версия</dt>
                  <dd>{s.version}</dd>
                </div>
                <div>
                  <dt>Ядро</dt>
                  <dd>{s.coreVersion}</dd>
                </div>
                <div>
                  <dt>Лицензия клиента</dt>
                  <dd>MIT · открытый исходный код</dd>
                </div>
              </dl>
              <p>
                VLESS / REALITY, VMess, Trojan, Shadowsocks, Hysteria2,
                WireGuard, SOCKS5 и HTTP(S). Импорт ссылок, WireGuard .conf,
                списков Base64 и сохраняемых HTTPS-подписок. Для доступа к
                интернету нужен собственный сервер или конфигурация провайдера.
              </p>
              <p className="quietnote">
                TUIC, OpenVPN, IKEv2, SSR, Clash YAML и SIP003-плагины пока не
                поддерживаются. Hysteria2 — без obfs и port hopping. WireGuard —
                один peer, без scripts/hooks. SOCKS5 и обычный HTTP сами по себе
                не шифруют трафик.
              </p>
            </div>
          )}
        </section>
        <section
          className={`connectionpane ${active ? "session-active" : ""}`}
          aria-label="Подключение"
        >
          <div className="connectionheader">
            <small className="eyebrow">СОЕДИНЕНИЕ</small>
            <span
              className={`status-pill ${s.connection.status === "connected" ? "online" : ""}`}
              role="status"
            >
              <i />
              {labels[s.connection.status] || "Отключён"}
            </span>
          </div>
          <div className="connectiontitle">
            <h2>
              {s.connection.status === "connected"
                ? "Вы на связи"
                : active
                  ? "Соединяем…"
                  : "Ваша сеть.\nВаш выбор."}
            </h2>
            <p>{display?.name || "Начните со своей конфигурации"}</p>
          </div>
          <div className="modeselect">{modePicker()}</div>
          {platform === "windows" &&
            s.settings.mode === "tun" &&
            !s.elevated && (
              <div className="adminnotice">
                <span>Для VPN всего ПК нужны права администратора.</span>
                <button
                  className="textbutton"
                  disabled={busy || active}
                  onClick={() => setModal("elevate")}
                >
                  Перезапустить с правами <Icon name="external" size={14} />
                </button>
              </div>
            )}
          <div className={`powerarea ${active ? "active" : ""}`}>
            <div className="powerouter">
              <button
                className={`power ${s.connection.status === "connecting" || s.connection.status === "reconnecting" ? "waiting" : ""}`}
                aria-label={active ? "Отключить" : "Подключить"}
                disabled={busy || s.connection.status === "disconnecting"}
                onClick={safe(async () => {
                  if (!active && !selected) {
                    openImport();
                    return;
                  }
                  if (
                    !active &&
                    platform === "windows" &&
                    s.settings.mode === "tun" &&
                    !s.elevated
                  ) {
                    setModal("elevate");
                    return;
                  }
                  setPowerRequest(active ? "disconnecting" : "connecting");
                  try {
                    await act(active ? "disconnect" : "connect");
                  } finally {
                    setPowerRequest(null);
                  }
                })}
              >
                <Icon name="power" size={38} />
                <span className="power-action">
                  {active
                    ? "Отключить"
                    : s.connection.status === "error"
                      ? "Повторить"
                      : "Подключить"}
                </span>
                <span className="power-status" role="status" aria-atomic="true">
                  {powerStatusLabel(powerRequest || s.connection.status)}
                </span>
              </button>
            </div>
          </div>
          <div className="sessionstrip">
            <div>
              <small>Время</small>
              <strong>
                {elapsed(active ? s.connection.connectedAt : null, clock)}
              </strong>
            </div>
            <div>
              <small>Получено</small>
              <strong>{bytes(s.connection.bytesDown)}</strong>
            </div>
            <div>
              <small>Отправлено</small>
              <strong>{bytes(s.connection.bytesUp)}</strong>
            </div>
          </div>
          {s.connection.error && (
            <p className="connectionerror" role="alert">
              {s.connection.error}
            </p>
          )}
          {s.settings.mode === "proxy" && (
            <p className="proxynote">
              {active && s.connection.httpPort
                ? `HTTP 127.0.0.1:${s.connection.httpPort} · SOCKS ${s.connection.socksPort}`
                : "Локальный прокси · только настроенные на него приложения"}
            </p>
          )}
          <div className="selectedserver">
            <span className="selectedicon">
              <Icon name="globe" size={35} />
            </span>
            <strong>{display?.name || "Сервер не выбран"}</strong>
            <span className="selectedprotocol">
              {display
                ? `${display.protocol.toUpperCase()} · ${display.transport.toUpperCase()}`
                : "Добавьте свою конфигурацию"}
            </span>
            <button
              className="primary pingbutton"
              disabled={
                !display ||
                pingBusy ||
                ["hysteria2", "wireguard"].includes(display?.protocol)
              }
              onClick={() => ping(display?.id)}
            >
              {pingBusy
                ? "Проверка…"
                : display && pings[display.id] != null
                  ? typeof pings[display.id] === "number"
                    ? `TCP: ${pings[display.id]} ms`
                    : pings[display.id]
                  : ["hysteria2", "wireguard"].includes(display?.protocol)
                    ? "UDP · без TCP-пинга"
                    : "Проверить TCP-пинг"}
            </button>
          </div>
        </section>
      </main>
      <nav
        className="mobilenav"
        aria-label="Основная навигация"
        inert={!!modal || drawer}
      >
        {[
          ["servers", "power", "Главная"],
          ["subscriptions", "link", "Подписки"],
          ["stats", "stats", "Сеанс"],
          ["settings", "settings", "Настройки"],
        ].map(([id, icon, title]) => (
          <button
            key={id}
            className={
              tab === id ||
              (id === "settings" && ["logs", "about"].includes(tab))
                ? "current"
                : ""
            }
            aria-current={tab === id ? "page" : undefined}
            onClick={() => {
              setTab(id);
              setMenu(null);
            }}
          >
            <Icon name={icon} size={22} />
            <span>{title}</span>
          </button>
        ))}
      </nav>
      {notice && (
        <div className="toast" role="status">
          <span>{notice}</span>
          <button
            className="iconbutton"
            aria-label="Закрыть уведомление"
            onClick={() => setNotice("")}
          >
            <Icon name="close" size={18} />
          </button>
        </div>
      )}
      {modal && (
        <div
          className="modalback"
          onClick={() => {
            if (!busy) setModal(null);
          }}
        >
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="dialog-title"
            onClick={(e) => e.stopPropagation()}
          >
            <header>
              <h2 id="dialog-title">
                {modal.details
                  ? "Сведения о сервере"
                  : modal.renameSubscription
                    ? "Название папки"
                    : modal === "add"
                      ? "Добавить подключение"
                      : modal === "elevate"
                        ? "Права для VPN (TUN)"
                        : modal === "import"
                          ? "Добавить сервер"
                          : modal === "subscription"
                            ? "Новая подписка"
                            : modal.removeSubscription
                              ? "Удалить подписку?"
                              : "Удалить сервер?"}
              </h2>
              <button
                className="iconbutton"
                aria-label="Закрыть"
                disabled={busy}
                onClick={() => setModal(null)}
              >
                <Icon name="close" />
              </button>
            </header>
            {modal.details ? (
              <dl>
                {[
                  ["Название", modal.details.name],
                  ["Протокол", modal.details.protocol.toUpperCase()],
                  ["Транспорт", modal.details.transport.toUpperCase()],
                  ["Адрес", modal.details.host],
                  ["Порт", modal.details.port],
                  [
                    "Источник",
                    subscriptions.find(
                      (x) => x.id === modal.details.subscriptionId,
                    )?.name || "Мои конфигурации",
                  ],
                ].map(([key, value]) => (
                  <div key={key}>
                    <dt>{key}</dt>
                    <dd>{value}</dd>
                  </div>
                ))}
              </dl>
            ) : modal.renameSubscription ? (
              <>
                <label className="fieldlabel">
                  Название
                  <input
                    autoFocus
                    value={subName}
                    maxLength={80}
                    onChange={(e) => setSubName(e.target.value)}
                  />
                </label>
                <footer>
                  <button
                    className="textbutton"
                    disabled={busy}
                    onClick={() => setModal(null)}
                  >
                    Отмена
                  </button>
                  <button
                    className="primary"
                    disabled={busy || !subName.trim()}
                    onClick={safe(async () => {
                      await act("subscription", {
                        action: "rename",
                        id: modal.renameSubscription.id,
                        name: subName.trim(),
                      });
                      setModal(null);
                    })}
                  >
                    Сохранить
                  </button>
                </footer>
              </>
            ) : modal === "add" ? (
              <div className="addchoices">
                <button
                  disabled={qrBusy}
                  onClick={() => qrInput.current?.click()}
                >
                  <Icon name="qr" />
                  <span>
                    <strong>
                      {qrBusy ? "Чтение QR-кода…" : "QR-код из изображения"}
                    </strong>
                    <small>
                      Открыть фото или скриншот · без загрузки в сеть
                    </small>
                  </span>
                  <Icon name="chevron" size={18} />
                </button>
                <button onClick={openSubscription}>
                  <Icon name="link" />
                  <span>
                    <strong>Подписка по ссылке</strong>
                    <small>Сохранить и обновлять список серверов</small>
                  </span>
                  <Icon name="chevron" size={18} />
                </button>
                <button onClick={safe(pasteImport)}>
                  <Icon name="copy" />
                  <span>
                    <strong>Из буфера обмена</strong>
                    <small>Распознаем ссылку или конфигурацию</small>
                  </span>
                  <Icon name="chevron" size={18} />
                </button>
                <button
                  onClick={() => {
                    setInput("");
                    setModal("import");
                  }}
                >
                  <Icon name="upload" />
                  <span>
                    <strong>Вручную или из файла</strong>
                    <small>Ссылки, Base64 и WireGuard .conf</small>
                  </span>
                  <Icon name="chevron" size={18} />
                </button>
              </div>
            ) : modal === "elevate" ? (
              <>
                <p>
                  Windows покажет запрос UAC, затем VXSStun перезапустится.
                  Профили сохранятся. VPN не включится автоматически — после
                  перезапуска нажмите «Подключить».
                </p>
                <footer>
                  <button
                    className="textbutton"
                    disabled={busy}
                    onClick={() => setModal(null)}
                  >
                    Отмена
                  </button>
                  <button
                    className="primary"
                    disabled={busy}
                    onClick={safe(() => act("elevate"))}
                  >
                    Перезапустить
                  </button>
                </footer>
              </>
            ) : modal === "import" || modal === "subscription" ? (
              <>
                <p>
                  {modal === "subscription"
                    ? "HTTPS-ссылка создаст подписку с обновлениями. Один ключ или список ключей создаст отдельную локальную папку."
                    : "VLESS, VMess, Trojan, Shadowsocks, Hysteria2, SOCKS5, HTTP(S) или WireGuard .conf. Можно вставить список ссылок."}
                </p>
                {modal === "subscription" && (
                  <label className="fieldlabel">
                    Название
                    <input
                      placeholder="Например, моя подписка"
                      value={subName}
                      maxLength={80}
                      onChange={(e) => setSubName(e.target.value)}
                    />
                  </label>
                )}
                {modal === "subscription" ? (
                  <label className="fieldlabel">
                    Ссылка подписки или ключи
                    <textarea
                      rows={3}
                      autoFocus
                      autoComplete="off"
                      autoCapitalize="none"
                      spellCheck={false}
                      placeholder="https://…"
                      value={input}
                      maxLength={262144}
                      onChange={(e) => setInput(e.target.value)}
                    />
                  </label>
                ) : (
                  <textarea
                    autoFocus
                    spellCheck="false"
                    autoComplete="off"
                    aria-label={
                      modal === "subscription"
                        ? "Ссылка подписки"
                        : "Конфигурации серверов"
                    }
                    placeholder={
                      modal === "subscription" ? "https://…" : "vless://…"
                    }
                    value={input}
                    onChange={(e) => setInput(e.target.value)}
                    maxLength={262144}
                  />
                )}
                <div className="importtools">
                  {modal === "import" && (
                    <button
                      className="textbutton"
                      onClick={() => {
                        setSubName("");
                        setModal("subscription");
                      }}
                    >
                      Это подписка? <Icon name="link" size={16} />
                    </button>
                  )}
                  <button
                    className="textbutton"
                    onClick={safe(() => pasteImport(modal === "subscription"))}
                  >
                    <Icon name="copy" size={17} />
                    Из буфера
                  </button>
                  <label
                    className="textbutton"
                    role="button"
                    tabIndex={0}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        e.currentTarget.querySelector("input").click();
                      }
                    }}
                  >
                    <Icon name="upload" size={17} />
                    Из файла
                    <input
                      type="file"
                      accept=".txt,.conf,text/plain"
                      hidden
                      onChange={async (e) => {
                        const el = e.currentTarget;
                        try {
                          const f = el.files[0];
                          if (f) {
                            if (f.size > 262144)
                              throw new Error("Файл больше 256 КиБ");
                            setInput(await f.text());
                          }
                        } catch (error) {
                          setNotice(error.message);
                        }
                        el.value = "";
                      }}
                    />
                  </label>
                </div>
                <footer>
                  <button
                    className="textbutton"
                    disabled={busy}
                    onClick={() => setModal(null)}
                  >
                    Отмена
                  </button>
                  <button
                    className="primary"
                    disabled={busy || !input.trim()}
                    onClick={safe(importNow)}
                  >
                    {busy ? "Добавление…" : "Добавить"}
                  </button>
                </footer>
              </>
            ) : (
              <>
                <p>
                  «{modal.removeSubscription?.name || modal.remove?.name}»{" "}
                  {modal.removeSubscription
                    ? "и все её серверы будут удалены"
                    : "будет удалён"}{" "}
                  с этого устройства.
                </p>
                <footer>
                  <button className="textbutton" onClick={() => setModal(null)}>
                    Отмена
                  </button>
                  <button
                    className="primary danger"
                    disabled={busy}
                    onClick={safe(async () => {
                      await act(
                        modal.removeSubscription ? "subscription" : "profile",
                        {
                          id: modal.removeSubscription?.id || modal.remove.id,
                          action: "remove",
                        },
                      );
                      if (modal.removeSubscription) setGroup("all");
                      setModal(null);
                    })}
                  >
                    Удалить
                  </button>
                </footer>
              </>
            )}
          </section>
        </div>
      )}
    </div>
  );
}
createRoot(document.getElementById("root")).render(<App />);
