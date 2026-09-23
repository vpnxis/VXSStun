import test from "node:test";
import assert from "node:assert/strict";
import {
  bytes,
  elapsed,
  isActive,
  importKind,
  powerStatusLabel,
} from "../src/format.js";
test("mobile button reports native state without claiming success on connect", () => {
  assert.equal(powerStatusLabel("disconnected"), "Подключить");
  assert.equal(powerStatusLabel("connecting"), "Подключение…");
  assert.equal(powerStatusLabel("connected"), "Подключён");
  assert.equal(powerStatusLabel("reconnecting"), "Восстановление…");
  assert.equal(powerStatusLabel("disconnecting"), "Отключение…");
  assert.equal(powerStatusLabel("error"), "Повторить");
  assert.equal(powerStatusLabel(undefined), "Подключить");
});
test("unknown is not invented traffic", () => assert.equal(bytes(null), "—"));
test("traffic units", () => {
  assert.equal(bytes(0), "0 Б");
  assert.equal(bytes(1048576), "1.0 МБ");
});
test("clipboard distinguishes subscription URLs from proxy profiles", () => {
  for (const v of [
    "https://example.invalid/sub?token=test",
    " https://example.invalid/ ",
    "https://example.invalid:8443/list",
  ])
    assert.equal(importKind(v), "subscription");
  for (const v of [
    "https://user:pass@example.invalid:443",
    "https://example.invalid:8443",
    "vless://x@example.invalid:443",
    "[Interface]\nPrivateKey=...",
    "aGVsbG8=",
    "vless://a\nvless://b",
  ])
    assert.equal(importKind(v), "profiles");
});
test("timer and clock rollback", () => {
  assert.equal(elapsed(100, 3761000), "01:01:01");
  assert.equal(elapsed(100, 90000), "00:00:00");
});
test("all transitional states stop instead of starting again", () => {
  for (const x of ["connecting", "connected", "reconnecting", "disconnecting"])
    assert.ok(isActive(x));
  assert.ok(!isActive("error"));
});
