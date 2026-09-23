// Development-only visual fixture. Vite removes its import from production.
// No real endpoints, profiles, secrets, persistence or successful connections.
if (new URLSearchParams(location.search).get("theme") === "dark") {
  await import("./preview-dark.css");
}
let previewSettings = {};
export function previewCall(method, payload, empty) {
  if (method !== "settings")
    throw new Error("Предпросмотр: реальные сетевые операции недоступны");
  previewSettings = { ...payload };
  return previewSnapshot(empty);
}
export function previewSnapshot(empty) {
  const q = new URLSearchParams(location.search);
  if (q.get("preview") !== "demo") return empty;
  const profiles = Array.from({ length: 18 }, (_, i) => ({
    id: `demo-${i}`,
    name:
      [
        "Стокгольм · Основной",
        "Хельсинки · Резервный",
        "Сервер с очень длинным названием для проверки интерфейса",
      ][i % 3] + (i > 2 ? ` ${i + 1}` : ""),
    protocol: ["vless", "trojan", "hysteria2"][i % 3],
    transport: ["tcp", "grpc", "quic"][i % 3],
    host: "example.invalid",
    port: 443,
    favorite: i === 0,
    subscriptionId: i < 3 ? null : i < 13 ? "demo-sub" : "demo-local",
  }));
  const state = q.get("state") || "disconnected";
  const status = [
    "disconnected",
    "connecting",
    "connected",
    "reconnecting",
    "disconnecting",
    "error",
  ].includes(state)
    ? state
    : "disconnected";
  return {
    ...empty,
    preview: true,
    profiles,
    subscriptions: [
      {
        id: "demo-sub",
        name: "Подписка с очень длинным названием для проверки узкого экрана",
        count: 10,
        kind: "remote",
        metadata: {
          upload: 1073741824,
          download: 2147483648,
          total: 53687091200,
          expire: 1900000000,
          announce: "Тестовые данные — без реальных серверов и ключей",
          updateIntervalHours: 12,
        },
        updatedAt: 1789740000,
      },
      {
        id: "demo-local",
        name: "Моя локальная папка",
        count: 5,
        kind: "local",
        updatedAt: 1789740000,
      },
    ],
    settings: {
      ...empty.settings,
      selected: "demo-0",
      mode: "proxy",
      ...previewSettings,
    },
    connection: {
      status,
      profileId: "demo-0",
      bytesDown: 20971520,
      bytesUp: 1048576,
      connectedAt:
        status === "connected" ? Math.floor(Date.now() / 1000) - 360 : null,
      error:
        status === "error"
          ? "Тестовое сообщение: сервер недоступен. Проверьте конфигурацию и подключение к сети."
          : null,
    },
  };
}
