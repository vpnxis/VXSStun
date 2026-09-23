export function filterProfiles(
  profiles,
  {
    query = "",
    favorites = false,
    protocol = "all",
    sort = "original",
    pings = {},
  } = {},
) {
  const needle = query.trim().toLocaleLowerCase("ru");
  const result = profiles.filter(
    (p) =>
      (!favorites || p.favorite) &&
      (protocol === "all" || p.protocol === protocol) &&
      `${p.name} ${p.protocol} ${p.transport}`
        .toLocaleLowerCase("ru")
        .includes(needle),
  );
  const name = (a, b) => a.name.localeCompare(b.name, "ru", { numeric: true });
  if (sort === "name") result.sort(name);
  if (sort === "protocol")
    result.sort((a, b) => a.protocol.localeCompare(b.protocol) || name(a, b));
  if (sort === "ping")
    result.sort(
      (a, b) =>
        (typeof pings[a.id] === "number" ? pings[a.id] : Infinity) -
          (typeof pings[b.id] === "number" ? pings[b.id] : Infinity) ||
        name(a, b),
    );
  return result;
}
export function libraryGroups(
  profiles,
  subscriptions,
  group = "all",
  onlySubscriptions = false,
) {
  const result = [];
  if (!onlySubscriptions && (group === "all" || group === "manual"))
    result.push({
      id: "manual",
      name: "Мои конфигурации",
      profiles: profiles.filter((p) => !p.subscriptionId),
      manual: true,
    });
  for (const sub of subscriptions)
    if (group === "all" || group === sub.id)
      result.push({
        ...sub,
        profiles: profiles.filter((p) => p.subscriptionId === sub.id),
      });
  return result;
}
export function fastestProfile(profiles, pings) {
  return profiles
    .filter((p) => Number.isFinite(pings[p.id]) && pings[p.id] >= 0)
    .sort((a, b) => pings[a.id] - pings[b.id])[0];
}
export function subscriptionUsage(meta = {}) {
  const known = Number.isFinite(meta.upload) || Number.isFinite(meta.download);
  const used = known
    ? Math.max(0, (meta.upload || 0) + (meta.download || 0))
    : null;
  const total =
    Number.isFinite(meta.total) && meta.total > 0 ? meta.total : null;
  return {
    used,
    total,
    percent: used != null && total ? Math.min(100, (used / total) * 100) : null,
    expired: meta.expire > 0 && meta.expire * 1000 < Date.now(),
  };
}

// Explicit opt-in in the caller. Never poll local folders or retry in a tight loop.
export function dueSubscriptions(subscriptions, now, attempts = {}) {
  return subscriptions.filter((sub) => {
    if (sub.kind === "local") return false;
    const hours = sub.metadata?.updateIntervalHours;
    const interval =
      (Number.isFinite(hours) ? Math.max(1, Math.min(168, hours)) : 24) *
      3600000;
    return (
      now - (sub.updatedAt || 0) * 1000 >= interval &&
      (attempts[sub.id] == null || now - attempts[sub.id] >= 300000)
    );
  });
}
