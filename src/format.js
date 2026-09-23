export function bytes(n) {
  if (n == null) return "—";
  const units = ["Б", "КБ", "МБ", "ГБ"];
  let v = Math.max(0, n),
    i = 0;
  while (v >= 1024 && i < 3) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(i && v < 100 ? 1 : 0)} ${units[i]}`;
}
export function elapsed(start, now = Date.now()) {
  const t = start ? Math.max(0, Math.floor(now / 1000 - start)) : 0;
  return [Math.floor(t / 3600), Math.floor(t / 60) % 60, t % 60]
    .map((v) => String(v).padStart(2, "0"))
    .join(":");
}
export function isActive(status) {
  return ["connecting", "connected", "reconnecting", "disconnecting"].includes(
    status,
  );
}
// The mobile button shows the session state, while its accessible name still
// describes the action (cancel/disconnect). Never infer "connected" from a tap.
export function powerStatusLabel(status) {
  return (
    {
      disconnected: "Подключить",
      connecting: "Подключение…",
      connected: "Подключён",
      reconnecting: "Восстановление…",
      disconnecting: "Отключение…",
      error: "Повторить",
    }[status] || "Подключить"
  );
}
// HTTPS proxy URIs contain credentials or a bare host:port; do not silently
// treat those as subscriptions. A URL with a path/query is a subscription.
export function importKind(text) {
  const value = text.trim();
  if (/\s/.test(value)) return "profiles";
  try {
    const url = new URL(value);
    if (
      url.protocol === "https:" &&
      !url.username &&
      !url.password &&
      !url.hash &&
      (!url.port || url.pathname !== "/" || url.search)
    )
      return "subscription";
  } catch {}
  return "profiles";
}
