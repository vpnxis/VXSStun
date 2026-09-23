import test from "node:test";
import assert from "node:assert/strict";
import {
  filterProfiles,
  libraryGroups,
  fastestProfile,
  subscriptionUsage,
  dueSubscriptions,
} from "../src/library.js";
const p = [
  {
    id: "a",
    name: "Zulu",
    protocol: "vless",
    transport: "tcp",
    favorite: true,
  },
  {
    id: "b",
    name: "Alpha",
    protocol: "trojan",
    transport: "ws",
    subscriptionId: "s",
  },
  {
    id: "c",
    name: "Other",
    protocol: "vless",
    transport: "tcp",
    subscriptionId: "s",
  },
];
test("standalone keys never merge into subscription folders", () => {
  const g = libraryGroups(p, [{ id: "s", name: "Sub" }]);
  assert.deepEqual(
    g.map((x) => x.profiles.map((p) => p.id)),
    [["a"], ["b", "c"]],
  );
  assert.equal(libraryGroups(p, [{ id: "s" }], "all", true).length, 1);
});
test("search, favorites, protocols and sorting are independent and immutable", () => {
  assert.deepEqual(
    filterProfiles(p, { favorites: true }).map((p) => p.id),
    ["a"],
  );
  assert.deepEqual(
    filterProfiles(p, { query: "ALP", protocol: "trojan" }).map((p) => p.id),
    ["b"],
  );
  assert.deepEqual(
    filterProfiles(p, { sort: "name" }).map((p) => p.id),
    ["b", "c", "a"],
  );
  assert.deepEqual(
    p.map((p) => p.id),
    ["a", "b", "c"],
  );
});
test("unmeasured/unreachable servers are never chosen as fastest", () => {
  const pings = { a: "Недоступен", b: 20, c: 80 };
  assert.equal(fastestProfile(p, pings).id, "b");
  assert.equal(fastestProfile(p, {}), undefined);
  assert.deepEqual(
    filterProfiles(p, { sort: "ping", pings }).map((p) => p.id),
    ["b", "c", "a"],
  );
});
test("traffic unknown, unlimited and exhausted are distinct", () => {
  assert.equal(subscriptionUsage().used, null);
  assert.equal(
    subscriptionUsage({ upload: 0, download: 0, total: 0 }).total,
    null,
  );
  assert.equal(
    subscriptionUsage({ upload: 10, download: 20, total: 25 }).percent,
    100,
  );
  assert.equal(subscriptionUsage({ expire: 1 }).expired, true);
});
test("auto refresh respects remote interval, local folders and retry backoff", () => {
  const now = 864000000;
  const list = [
    {
      id: "a",
      updatedAt: now / 1000 - 3600,
      metadata: { updateIntervalHours: 1 },
    },
    { id: "b", updatedAt: now / 1000 - 3600 },
    { id: "local", kind: "local" },
  ];
  assert.deepEqual(
    dueSubscriptions(list, now).map((s) => s.id),
    ["a"],
  );
  assert.deepEqual(dueSubscriptions(list, now, { a: now - 299999 }), []);
  assert.deepEqual(
    dueSubscriptions(list, now, { a: now - 300000 }).map((s) => s.id),
    ["a"],
  );
});
