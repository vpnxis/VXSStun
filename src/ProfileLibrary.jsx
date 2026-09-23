import React, { useState } from "react";
import { bytes } from "./format";
import { libraryGroups, fastestProfile, subscriptionUsage } from "./library";

export function SubscriptionInfo({ sub }) {
  const meta = sub.metadata || {},
    usage = subscriptionUsage(meta);
  return (
    <>
      {(usage.used != null || meta.expire > 0) && (
        <div className="subscription-info">
          {usage.used != null && (
            <span>
              {bytes(usage.used)} /{" "}
              {usage.total
                ? bytes(usage.total)
                : meta.total === 0
                  ? "без лимита"
                  : "лимит не передан"}
            </span>
          )}
          {meta.expire > 0 && (
            <span className={usage.expired ? "expired" : ""}>
              {usage.expired ? "Истекла" : "До"}{" "}
              {new Date(meta.expire * 1000).toLocaleDateString("ru-RU")}
            </span>
          )}
          {usage.percent != null && (
            <progress
              aria-label={`Использовано трафика: ${sub.name}`}
              max="100"
              value={usage.percent}
            />
          )}
        </div>
      )}
      {meta.announce && (
        <p className="subscription-announcement">{meta.announce}</p>
      )}
    </>
  );
}

export default function ProfileLibrary({
  Icon,
  profiles,
  allProfiles,
  subscriptions,
  group = "all",
  onlySubscriptions = false,
  selected,
  connected,
  pings,
  busy,
  pingBusy,
  menu,
  setMenu,
  onSelect,
  onFavorite,
  onPing,
  onPingGroup,
  onRefresh,
  onRename,
  onDetails,
  onRemove,
  onRemoveSubscription,
  onAdd,
}) {
  const [folded, setFolded] = useState({});
  const groups = libraryGroups(
    profiles,
    subscriptions,
    group,
    onlySubscriptions,
  );
  const hasFilter = profiles.length !== allProfiles.length;
  const toggle = (id) => setFolded((old) => ({ ...old, [id]: !old[id] }));
  return (
    <div className="profile-library">
      <div className="library-tools">
        <span>
          {groups.reduce((count, group) => count + group.profiles.length, 0)}{" "}
          профилей
        </span>
        <button
          className="textbutton"
          onClick={() =>
            setFolded(Object.fromEntries(groups.map((g) => [g.id, true])))
          }
        >
          Свернуть всё
        </button>
        <button className="textbutton" onClick={() => setFolded({})}>
          Раскрыть всё
        </button>
      </div>
      {groups.map((g) => {
        const total = allProfiles.filter((p) =>
          g.manual ? !p.subscriptionId : p.subscriptionId === g.id,
        ).length;
        const fastest = fastestProfile(g.profiles, pings);
        return (
          <article
            key={g.id}
            className={`profile-folder ${g.manual ? "manual-folder" : "subscription-folder"}`}
          >
            <header className="folder-header">
              <button
                className="folder-title"
                onClick={() => toggle(g.id)}
                aria-expanded={!folded[g.id]}
              >
                <Icon name={g.manual ? "globe" : "folder"} size={23} />
                <span>
                  <strong>{g.name}</strong>
                  <small>
                    {hasFilter ? `${g.profiles.length} из ${total}` : total}{" "}
                    {g.manual
                      ? "конфигураций"
                      : `серверов · ${g.kind === "local" ? "локальная папка" : "подписка"}`}
                  </small>
                </span>
                <Icon name={folded[g.id] ? "chevron" : "down"} size={17} />
              </button>
              <div className="folder-actions">
                {!g.manual && g.kind !== "local" && (
                  <button
                    className="iconbutton"
                    disabled={busy}
                    aria-label={`Обновить ${g.name}`}
                    onClick={() => onRefresh(g)}
                  >
                    <Icon name="refresh" size={19} />
                  </button>
                )}
                <button
                  className="iconbutton"
                  disabled={pingBusy || !g.profiles.length}
                  aria-label={`TCP-пинг: ${g.name}`}
                  onClick={() => onPingGroup(g.profiles)}
                >
                  <Icon name="ping" size={20} />
                </button>
                {!g.manual && (
                  <button
                    className="iconbutton"
                    aria-label={`Параметры папки ${g.name}`}
                    onClick={() => setMenu(menu === g.id ? null : g.id)}
                  >
                    <Icon name="more" size={20} />
                  </button>
                )}
                {menu === g.id && (
                  <div className="popover">
                    <button
                      onClick={() => {
                        setMenu(null);
                        onRename(g);
                      }}
                    >
                      <Icon name="edit" size={18} />
                      Переименовать
                    </button>
                    <button
                      onClick={() => {
                        setMenu(null);
                        onRemoveSubscription(g);
                      }}
                    >
                      <Icon name="trash" size={18} />
                      Удалить папку
                    </button>
                  </div>
                )}
              </div>
            </header>
            {!folded[g.id] && (
              <div className="folder-body">
                {!g.manual && (
                  <>
                    <SubscriptionInfo sub={g} />
                    <div className="folder-meta">
                      <span>
                        {g.updatedAt
                          ? `Обновлена ${new Date(g.updatedAt * 1000).toLocaleString("ru-RU", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" })}`
                          : "Ещё не обновлялась"}
                      </span>
                      {g.metadata?.updateIntervalHours > 0 && (
                        <span>
                          Интервал провайдера: {g.metadata.updateIntervalHours}{" "}
                          ч
                        </span>
                      )}
                    </div>
                  </>
                )}
                {fastest && g.profiles.length > 1 && (
                  <button
                    className="textbutton fastest"
                    disabled={busy}
                    onClick={() => onSelect(fastest)}
                  >
                    Выбрать минимальный TCP-пинг · {pings[fastest.id]} мс
                  </button>
                )}
                {g.profiles.map((p) => (
                  <div
                    key={p.id}
                    className={`serverrow ${p.id === selected ? "selected" : ""}`}
                  >
                    <button
                      className="serverselect"
                      disabled={busy}
                      aria-pressed={p.id === selected}
                      onClick={() => onSelect(p)}
                    >
                      <span className="servericon">
                        <Icon
                          name={p.id === selected ? "check" : "globe"}
                          size={22}
                        />
                      </span>
                      <span className="servertext">
                        <span>{p.name}</span>
                        <small>
                          {p.protocol.toUpperCase()} ·{" "}
                          {p.transport.toUpperCase()}
                          {p.favorite ? " · ★" : ""}
                          {p.id === connected ? " · В работе" : ""}
                        </small>
                      </span>
                      <span className="latency">
                        {typeof pings[p.id] === "number"
                          ? `${pings[p.id]} мс`
                          : pings[p.id] || ""}
                      </span>
                    </button>
                    <button
                      className="iconbutton rowmenu"
                      aria-label={`Действия: ${p.name}`}
                      onClick={() => setMenu(menu === p.id ? null : p.id)}
                    >
                      <Icon name="more" size={19} />
                    </button>
                    {menu === p.id && (
                      <div className="popover">
                        <button
                          onClick={() => {
                            setMenu(null);
                            onDetails(p);
                          }}
                        >
                          <Icon name="info" size={18} />
                          Сведения
                        </button>
                        <button
                          disabled={
                            pingBusy ||
                            ["hysteria2", "wireguard"].includes(p.protocol)
                          }
                          onClick={() => {
                            setMenu(null);
                            onPing(p.id);
                          }}
                        >
                          <Icon name="ping" size={18} />
                          TCP-пинг
                        </button>
                        <button
                          disabled={busy}
                          onClick={() => {
                            setMenu(null);
                            onFavorite(p);
                          }}
                        >
                          <Icon name="star" size={18} />
                          {p.favorite ? "Из избранного" : "В избранное"}
                        </button>
                        <button
                          disabled={busy}
                          onClick={() => {
                            setMenu(null);
                            onRemove(p);
                          }}
                        >
                          <Icon name="trash" size={18} />
                          Удалить
                        </button>
                      </div>
                    )}
                  </div>
                ))}
                {!g.profiles.length && (
                  <div className="folder-empty">
                    {hasFilter
                      ? "Нет совпадений в этой папке"
                      : g.manual
                        ? "Одиночные ключи появятся здесь"
                        : "В папке пока нет серверов"}
                    {g.manual && !hasFilter && (
                      <button className="textbutton" onClick={onAdd}>
                        Добавить конфигурацию
                      </button>
                    )}
                  </div>
                )}
              </div>
            )}
          </article>
        );
      })}
    </div>
  );
}
