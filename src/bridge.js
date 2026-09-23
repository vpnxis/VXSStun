import { invoke } from "@tauri-apps/api/core";
const pending = new Map();
let serial = 0;
window.vxsResult = (id, ok, value) => {
  const p = pending.get(id);
  if (!p) return;
  clearTimeout(p.timer);
  pending.delete(id);
  ok ? p.resolve(value) : p.reject(new Error(value));
};
export const platform = window.VXSNative
  ? "android"
  : window.__TAURI_INTERNALS__
    ? "windows"
    : "preview";
export const emptySnapshot = () => ({
  profiles: [],
  subscriptions: [],
  settings: {
    theme: "system",
    mode: platform === "android" ? "tun" : "proxy",
    reconnect: false,
    autoUpdateSubscriptions: false,
    selected: null,
  },
  connection: { status: "disconnected" },
  events: [],
  version: "0.0.1.alpha.1",
  coreVersion: "Xray",
  elevated: false,
});
export async function call(method, payload = {}) {
  if (platform === "android")
    return new Promise((resolve, reject) => {
      const id = String(++serial);
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error("Операция не завершилась за 45 секунд"));
      }, 45000);
      pending.set(id, { resolve, reject, timer });
      window.VXSNative.request(id, method, JSON.stringify(payload));
    });
  if (platform === "windows") return invoke("dispatch", { method, payload });
  if (method === "snapshot") {
    if (import.meta.env.DEV)
      return (await import("./preview.js")).previewSnapshot(emptySnapshot());
    return emptySnapshot();
  }
  if (import.meta.env.DEV && method === "settings")
    return (await import("./preview.js")).previewCall(
      method,
      payload,
      emptySnapshot(),
    );
  throw new Error(
    "Это предпросмотр интерфейса. Импорт и VPN доступны в APK и EXE.",
  );
}
