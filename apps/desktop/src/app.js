const { invoke } = window.__TAURI__.core;
const tauriEvent = window.__TAURI__?.event;

const MODE_LABELS = {
  observe: "Observe",
  moderate: "Moderate",
  cookie_monster: "Cookie Monster",
};

const TIPS = {
  mode: {
    title: "Modes",
    body: "Observe logs only. Moderate allows Necessary (+ first-party Preferences) and allowlisted hosts. Cookie Monster denies sets and strips sends.",
  },
  allowlist: {
    title: "Allowlist",
    body: "Comma-separated hosts (e.g. github.com). Subdomains match. Used in Moderate mode.",
  },
  shields: {
    title: "Shields",
    body: "Browser shield = extensions ↔ core. System shield = local proxy with on-device HTTPS MITM after you trust the Local CA.",
  },
  feed: {
    title: "Activity",
    body: "Search, filter, and sort redacted Set/Send events. Click a row for details. Cookie values never leave this device.",
  },
  explore: {
    title: "Explore",
    body: "Switch between By host (default — hunt trackers) and By category (privacy buckets). Also group by decision or cookie name. Open a group for details and allowlist.",
  },
  views: {
    title: "Views & sorts",
    body: "Saved defaults for Explore grouping and Activity sorting. Host stays the default; category is always one click away. Changes apply immediately and persist on this device.",
  },
  setup: {
    title: "Setup",
    body: "Checklist of local setup steps. All data stays on this device — nothing is uploaded. Progress is saved and skipped after updates.",
  },
  data: {
    title: "Data",
    body: "Encrypted event log on this device. Clearing activity keeps your key, policy, and allowlist. Auto-prune deletes older events only.",
  },
  system: {
    title: "System",
    body: "Proxy routes HTTP/HTTPS through 127.0.0.1:8877 when System shield is on. Restart core reloads the native host if it drops.",
  },
  reset: {
    title: "Reset setup",
    body: "Re-shows the first-run wizard only. Does not clear the activity log, keys, or allowlist.",
  },
};

const WIZARD_STEPS = [
  {
    id: "welcome",
    title: "Welcome to Cookie Monster",
    body: "Everything stays on this device — no cloud. We’ll start the local core, optionally trust a Local CA, load a browser extension, and optionally enable the system shield.",
    primary: "Get started",
    secondary: null,
  },
  {
    id: "host",
    title: "Start the local core",
    body: "Starts the encrypted log + policy engine on this Mac.",
    primary: "Start core",
    secondary: null,
    action: "ensure_host",
    flag: "host_done",
  },
  {
    id: "ca",
    title: "Trust the Local CA",
    body: "Needed for HTTPS cookie decisions. macOS may ask for your password. Skip to use browser shield only.",
    primary: "Trust certificate",
    secondary: "Skip for now",
    action: "install_ca",
    flag: "ca_done",
  },
  {
    id: "extension",
    title: "Load the browser extension",
    body: "Chrome: chrome://extensions → Load unpacked. Firefox: about:debugging → Load Temporary Add-on.",
    primary: "Open extension folder",
    secondary: "I’ve done this / Skip",
    action: "open_extension",
    flag: "extension_done",
  },
  {
    id: "system",
    title: "System shield (optional)",
    body: "Routes HTTP/HTTPS through 127.0.0.1:8877. You can toggle this anytime from Control or Settings.",
    primary: "Enable + set proxy",
    secondary: "Skip",
    action: "system_shield",
    flag: "system_shield_done",
  },
  {
    id: "views",
    title: "Host & category views",
    body: "Explore defaults to By host (find trackers). Switch to By category anytime for privacy buckets. Both stay in the app — Settings saves your preferred group and sorts.",
    primary: "Open Explore (host)",
    secondary: "I’ve got it",
    action: "open_views",
    flag: "views_done",
  },
  {
    id: "done",
    title: "You’re set",
    body: "Setup is saved locally and won’t ask again after updates. Use Settings to clear logs or reset setup.",
    primary: "Finish",
    secondary: null,
  },
];

const modeEl = document.getElementById("mode");
const errorEl = document.getElementById("error");
const feedEl = document.getElementById("feed");
const feedEmpty = document.getElementById("feed-empty");
const feedSearchEl = document.getElementById("feed-search");
const feedFilterDecisionEl = document.getElementById("feed-filter-decision");
const feedFilterActionEl = document.getElementById("feed-filter-action");
const feedFilterBucketEl = document.getElementById("feed-filter-bucket");
const feedSortEl = document.getElementById("feed-sort");
const feedCountEl = document.getElementById("feed-count");
const allowlistEl = document.getElementById("allowlist");
const systemShieldEl = document.getElementById("system-shield");
const tip = document.getElementById("tip");
const tipTitle = document.getElementById("tip-title");
const tipBody = document.getElementById("tip-body");
const shieldModal = document.getElementById("shield-modal");
const wizard = document.getElementById("wizard");
const wizardTitle = document.getElementById("wizard-title");
const wizardBody = document.getElementById("wizard-body");
const wizardProgress = document.getElementById("wizard-progress");
const wizardStatus = document.getElementById("wizard-status");
const wizardPrimary = document.getElementById("wizard-primary");
const wizardSecondary = document.getElementById("wizard-secondary");
const storeStatsEl = document.getElementById("store-stats");
const retentionEl = document.getElementById("retention-days");
const statusStripEl = document.getElementById("status-strip");
const exploreCatsEl = document.getElementById("explore-cats");
const exploreEmptyEl = document.getElementById("explore-empty");
const exploreSummaryEl = document.getElementById("explore-summary");
const exploreDetailEl = document.getElementById("explore-detail");
const exploreDetailTitle = document.getElementById("explore-detail-title");
const exploreDetailList = document.getElementById("explore-detail-list");
const exploreSortEl = document.getElementById("explore-sort");
const exploreHostCountEl = document.getElementById("explore-host-count");
const exploreBucketCountEl = document.getElementById("explore-bucket-count");
const prefExploreGroupEl = document.getElementById("pref-explore-group");
const prefExploreSortEl = document.getElementById("pref-explore-sort");
const prefActivitySortEl = document.getElementById("pref-activity-sort");
const eventModal = document.getElementById("event-modal");
const eventModalTitle = document.getElementById("event-modal-title");
const eventModalBody = document.getElementById("event-modal-body");

const VALID_GROUPS = new Set(["host", "bucket", "decision", "cookie"]);
const VALID_EXPLORE_SORTS = new Set(["count", "name", "recent", "denied", "host", "bucket"]);
const VALID_ACTIVITY_SORTS = new Set([
  "newest",
  "oldest",
  "host",
  "bucket",
  "name",
  "decision",
]);

let allowlistLoaded = false;
let suppressShieldEvent = false;
let prefsReady = false;
let wizardIndex = 0;
let wizardBusy = false;
/** @type {Array<{host:string,name:string,action:string,decision:string,bucket:string,stored_at_ms:number}>} */
let feedEvents = [];
let exploreGroupBy = "host";
/** @type {{ key: string, label: string, events: typeof feedEvents } | null} */
let exploreActiveGroup = null;
/** @type {typeof feedEvents[0] | null} */
let selectedEvent = null;
let setupState = {
  schema_version: 2,
  welcome_done: false,
  host_done: false,
  ca_done: false,
  extension_done: false,
  system_shield_done: false,
  views_done: false,
  completed: false,
  explore_group_by: "host",
  explore_sort: "count",
  activity_sort: "newest",
};

function showTip(key) {
  const tipData = TIPS[key];
  if (!tipData) return;
  tipTitle.textContent = tipData.title;
  tipBody.textContent = tipData.body;
  tip.hidden = false;
}

function hideTip() {
  tip.hidden = true;
}

function setStep(name, done) {
  document.querySelector(`[data-step="${name}"]`)?.classList.toggle("done", done);
}

function updateSetupChecklist(snapshot) {
  setStep("host", !!snapshot.connected || setupState.host_done);
  setStep("ca", setupState.ca_done);
  setStep("extension", setupState.extension_done);
  setStep("proxy", !!snapshot.system_shield || setupState.system_shield_done);
  setStep("views", !!setupState.views_done);
}

function normalizePrefs(state) {
  const next = { ...setupState, ...state };
  if (!VALID_GROUPS.has(next.explore_group_by)) next.explore_group_by = "host";
  if (!VALID_EXPLORE_SORTS.has(next.explore_sort)) next.explore_sort = "count";
  if (!VALID_ACTIVITY_SORTS.has(next.activity_sort)) next.activity_sort = "newest";
  if (typeof next.views_done !== "boolean") next.views_done = false;
  return next;
}

function applyPrefsToControls() {
  exploreGroupBy = setupState.explore_group_by || "host";
  if (exploreSortEl && VALID_EXPLORE_SORTS.has(setupState.explore_sort)) {
    exploreSortEl.value = setupState.explore_sort;
  }
  if (feedSortEl && VALID_ACTIVITY_SORTS.has(setupState.activity_sort)) {
    feedSortEl.value = setupState.activity_sort;
  }
  if (prefExploreGroupEl) prefExploreGroupEl.value = exploreGroupBy;
  if (prefExploreSortEl) prefExploreSortEl.value = setupState.explore_sort || "count";
  if (prefActivitySortEl) prefActivitySortEl.value = setupState.activity_sort || "newest";
  syncExploreGroupUi();
}

function syncExploreGroupUi() {
  document.querySelectorAll(".explore-group-toggle .chip").forEach((chip) => {
    chip.classList.toggle("active", chip.dataset.group === exploreGroupBy);
  });
  document.querySelectorAll(".explore-dual .dual-card").forEach((card) => {
    card.classList.toggle("active", card.dataset.group === exploreGroupBy);
  });
}

async function saveViewPrefs(partial = {}) {
  setupState = normalizePrefs({ ...setupState, ...partial });
  applyPrefsToControls();
  await persistSetup();
}

async function setExploreGroup(group, { persist = true, markViews = false } = {}) {
  const next = VALID_GROUPS.has(group) ? group : "host";
  exploreGroupBy = next;
  exploreActiveGroup = null;
  syncExploreGroupUi();
  if (persist || markViews) {
    const patch = { explore_group_by: next };
    if (markViews) patch.views_done = true;
    await saveViewPrefs(patch);
  }
  renderExplore();
}

function showError(msg) {
  errorEl.hidden = false;
  errorEl.textContent = msg;
}

function render(snapshot) {
  const online = !!snapshot.connected;
  modeEl.textContent = online
    ? MODE_LABELS[snapshot.mode] ?? snapshot.mode
    : snapshot.error
      ? "Offline"
      : "Starting…";
  modeEl.dataset.state =
    !online ? "offline" : snapshot.mode === "cookie_monster" ? "deny" : "on";

  const hideErr =
    !snapshot.error ||
    /connection refused|host not running|host unreachable/i.test(snapshot.error || "");
  errorEl.hidden = hideErr || online;
  if (!hideErr) errorEl.textContent = snapshot.error ?? "";
  if (online) errorEl.hidden = true;

  document.querySelectorAll(".modes button").forEach((btn) => {
    btn.classList.toggle("active", online && btn.dataset.mode === snapshot.mode);
    btn.disabled = !online;
  });

  suppressShieldEvent = true;
  systemShieldEl.checked = !!snapshot.system_shield;
  systemShieldEl.disabled = !online;
  suppressShieldEvent = false;

  updateSetupChecklist(snapshot);

  feedEvents = snapshot.events ?? [];
  refreshBucketFilterOptions();
  renderFeed();
  renderExplore();
  updateStatusStrip(snapshot);
}

function updateStatusStrip(snapshot) {
  if (!statusStripEl) return;
  const host = statusStripEl.querySelector('[data-k="host"]');
  const shield = statusStripEl.querySelector('[data-k="shield"]');
  const events = statusStripEl.querySelector('[data-k="events"]');
  const online = !!snapshot.connected;
  if (host) {
    host.textContent = online ? "Host ON" : "Host OFF";
    host.dataset.on = online ? "1" : "0";
  }
  if (shield) {
    shield.textContent = snapshot.system_shield ? "Shield ON" : "Shield OFF";
    shield.dataset.on = snapshot.system_shield ? "1" : "0";
  }
  if (events) {
    const n = (snapshot.events ?? []).length;
    events.textContent = `${n} event${n === 1 ? "" : "s"}`;
    events.dataset.on = n > 0 ? "1" : "";
  }
}

function feedQuery() {
  return (feedSearchEl?.value ?? "").trim().toLowerCase();
}

function refreshBucketFilterOptions() {
  if (!feedFilterBucketEl) return;
  const prev = feedFilterBucketEl.value || "all";
  const buckets = [...new Set(feedEvents.map((e) => e.bucket).filter(Boolean))].sort((a, b) =>
    a.localeCompare(b, undefined, { sensitivity: "base" }),
  );
  feedFilterBucketEl.replaceChildren(
    Object.assign(document.createElement("option"), {
      value: "all",
      textContent: "All categories",
    }),
    ...buckets.map((b) =>
      Object.assign(document.createElement("option"), { value: b, textContent: b }),
    ),
  );
  feedFilterBucketEl.value = buckets.includes(prev) || prev === "all" ? prev : "all";
}

function filteredFeedEvents() {
  const q = feedQuery();
  const decision = feedFilterDecisionEl?.value ?? "all";
  const action = feedFilterActionEl?.value ?? "all";
  const bucket = feedFilterBucketEl?.value ?? "all";
  const sort = feedSortEl?.value ?? "newest";

  let list = feedEvents.filter((e) => {
    if (decision !== "all" && e.decision !== decision) return false;
    if (action !== "all" && e.action !== action) return false;
    if (bucket !== "all" && e.bucket !== bucket) return false;
    if (!q) return true;
    const hay = `${e.host} ${e.name} ${e.action} ${e.decision} ${e.bucket}`.toLowerCase();
    return hay.includes(q);
  });

  const cmpStr = (a, b) => a.localeCompare(b, undefined, { sensitivity: "base" });
  list = [...list].sort((a, b) => {
    switch (sort) {
      case "oldest":
        return a.stored_at_ms - b.stored_at_ms;
      case "host":
        return cmpStr(a.host, b.host) || b.stored_at_ms - a.stored_at_ms;
      case "bucket":
        return cmpStr(a.bucket || "", b.bucket || "") || b.stored_at_ms - a.stored_at_ms;
      case "name":
        return cmpStr(a.name, b.name) || b.stored_at_ms - a.stored_at_ms;
      case "decision":
        return cmpStr(a.decision, b.decision) || b.stored_at_ms - a.stored_at_ms;
      case "newest":
      default:
        return b.stored_at_ms - a.stored_at_ms;
    }
  });
  return list;
}

function eventRow(e, onClick) {
  const li = document.createElement("li");
  li.tabIndex = 0;
  li.append(
    Object.assign(document.createElement("span"), { className: "host", textContent: e.host }),
    Object.assign(document.createElement("span"), {
      className: `decision ${e.decision}`,
      textContent: e.decision,
    }),
    Object.assign(document.createElement("span"), {
      className: "meta",
      textContent: `${e.name} · ${e.action} · ${e.bucket} · ${new Date(e.stored_at_ms).toLocaleTimeString()}`,
    }),
  );
  li.addEventListener("click", () => onClick(e));
  li.addEventListener("keydown", (ev) => {
    if (ev.key === "Enter" || ev.key === " ") {
      ev.preventDefault();
      onClick(e);
    }
  });
  return li;
}

function renderFeed() {
  const events = filteredFeedEvents();
  const total = feedEvents.length;
  const showing = events.length;
  if (feedCountEl) {
    if (total === 0) {
      feedCountEl.textContent = "";
    } else if (showing === total) {
      feedCountEl.textContent = `${total} event${total === 1 ? "" : "s"} · click for details`;
    } else {
      feedCountEl.textContent = `Showing ${showing} of ${total} · click for details`;
    }
  }
  feedEmpty.hidden = events.length > 0;
  feedEmpty.textContent =
    total === 0
      ? "No events yet"
      : "No events match this search / filter";
  feedEl.replaceChildren(...events.map((e) => eventRow(e, openEventDetail)));
}

/**
 * Group activity for Explore. Key = host | bucket | decision | cookie name.
 * Sort/weight choices change what “noisy” trackers surface first.
 */
function buildExploreGroups(events, groupBy) {
  const map = new Map();
  for (const e of events) {
    let key = e.host || "(unknown)";
    if (groupBy === "bucket") key = e.bucket || "(uncategorized)";
    else if (groupBy === "decision") key = e.decision || "(unknown)";
    else if (groupBy === "cookie") key = e.name || "(unnamed)";
    let g = map.get(key);
    if (!g) {
      g = { key, label: key, events: [], denied: 0, latest: 0 };
      map.set(key, g);
    }
    g.events.push(e);
    if (e.decision === "Deny" || e.decision === "Strip") g.denied += 1;
    if (e.stored_at_ms > g.latest) g.latest = e.stored_at_ms;
  }
  return [...map.values()];
}

function sortExploreGroups(groups, sort) {
  const byName = (a, b) => a.label.localeCompare(b.label, undefined, { sensitivity: "base" });
  const firstHost = (g) => g.events[0]?.host || g.label;
  const firstBucket = (g) => g.events[0]?.bucket || g.label;
  return [...groups].sort((a, b) => {
    switch (sort) {
      case "name":
      case "host":
        return sort === "host"
          ? firstHost(a).localeCompare(firstHost(b), undefined, { sensitivity: "base" })
          : byName(a, b);
      case "bucket":
        return firstBucket(a).localeCompare(firstBucket(b), undefined, { sensitivity: "base" })
          || byName(a, b);
      case "recent":
        return b.latest - a.latest || b.events.length - a.events.length;
      case "denied":
        return b.denied - a.denied || b.events.length - a.events.length;
      case "count":
      default:
        return b.events.length - a.events.length || byName(a, b);
    }
  });
}

function updateExploreDualCounts() {
  const hosts = buildExploreGroups(feedEvents, "host").length;
  const cats = buildExploreGroups(feedEvents, "bucket").length;
  if (exploreHostCountEl) {
    exploreHostCountEl.textContent = `${hosts} host${hosts === 1 ? "" : "s"}`;
  }
  if (exploreBucketCountEl) {
    exploreBucketCountEl.textContent = `${cats} categor${cats === 1 ? "y" : "ies"}`;
  }
}

function renderExplore() {
  if (!exploreCatsEl) return;
  updateExploreDualCounts();
  syncExploreGroupUi();
  if (exploreActiveGroup) {
    renderExploreDetail();
    return;
  }
  const groups = sortExploreGroups(
    buildExploreGroups(feedEvents, exploreGroupBy),
    exploreSortEl?.value ?? setupState.explore_sort ?? "count",
  );
  const viewLabel =
    exploreGroupBy === "bucket"
      ? "category"
      : exploreGroupBy === "host"
        ? "host"
        : exploreGroupBy;
  if (exploreSummaryEl) {
    exploreSummaryEl.textContent =
      feedEvents.length === 0
        ? ""
        : `Viewing by ${viewLabel} · ${groups.length} group${groups.length === 1 ? "" : "s"} · ${feedEvents.length} events`;
  }
  if (exploreEmptyEl) exploreEmptyEl.hidden = groups.length > 0;
  if (exploreDetailEl) exploreDetailEl.hidden = true;
  exploreCatsEl.hidden = false;
  exploreCatsEl.replaceChildren(
    ...groups.map((g) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "cat-card";
      const deniedBit = g.denied ? ` · ${g.denied} blocked` : "";
      btn.append(
        Object.assign(document.createElement("span"), {
          className: "cat-name",
          textContent: g.label,
        }),
        Object.assign(document.createElement("span"), {
          className: "cat-count",
          textContent: String(g.events.length),
        }),
        Object.assign(document.createElement("span"), {
          className: "cat-meta",
          textContent: `${g.events.length} event${g.events.length === 1 ? "" : "s"}${deniedBit}`,
        }),
      );
      btn.addEventListener("click", () => {
        exploreActiveGroup = g;
        renderExplore();
      });
      return btn;
    }),
  );
}

function renderExploreDetail() {
  if (!exploreActiveGroup || !exploreDetailEl) return;
  exploreCatsEl.hidden = true;
  if (exploreEmptyEl) exploreEmptyEl.hidden = true;
  exploreDetailEl.hidden = false;
  exploreDetailTitle.textContent = exploreActiveGroup.label;
  const list = [...exploreActiveGroup.events].sort(
    (a, b) => b.stored_at_ms - a.stored_at_ms,
  );
  exploreDetailList.replaceChildren(...list.map((e) => eventRow(e, openEventDetail)));
  if (exploreSummaryEl) {
    exploreSummaryEl.textContent = `${list.length} event${list.length === 1 ? "" : "s"} in group`;
  }
}

function openEventDetail(e) {
  selectedEvent = e;
  eventModalTitle.textContent = e.name || "Cookie event";
  const rows = [
    ["Host", e.host],
    ["Cookie", e.name],
    ["Action", e.action],
    ["Decision", e.decision],
    ["Category", e.bucket],
    ["When", new Date(e.stored_at_ms).toLocaleString()],
  ];
  eventModalBody.replaceChildren(
    ...rows.flatMap(([k, v]) => [
      Object.assign(document.createElement("dt"), { textContent: k }),
      Object.assign(document.createElement("dd"), { textContent: v || "—" }),
    ]),
  );
  eventModal.hidden = false;
}

function closeEventDetail() {
  eventModal.hidden = true;
  selectedEvent = null;
}

async function allowlistSelectedHost() {
  if (!selectedEvent?.host) return;
  const host = selectedEvent.host.replace(/^\./, "");
  const current = allowlistEl.value
    .split(/[,\n ]+/)
    .map((s) => s.trim())
    .filter(Boolean);
  if (!current.some((h) => h.toLowerCase() === host.toLowerCase())) {
    current.push(host);
    allowlistEl.value = current.join(", ");
  }
  try {
    await invoke("set_allowlist", { hostsCsv: allowlistEl.value });
    allowlistLoaded = true;
    closeEventDetail();
    activateTab("control");
    errorEl.hidden = true;
  } catch (err) {
    showError(String(err));
  }
}

async function loadAllowlist() {
  if (allowlistLoaded) return;
  try {
    const hosts = await invoke("get_allowlist");
    allowlistEl.value = (hosts ?? []).join(", ");
    allowlistLoaded = true;
  } catch {
  }
}

async function refreshStats() {
  try {
    const stats = await invoke("get_store_stats");
    storeStatsEl.textContent = `${stats.event_count} encrypted events · ${stats.retention_days}-day auto-prune · local only`;
    if (retentionEl && String(stats.retention_days) !== retentionEl.value) {
      const opt = Array.from(retentionEl.options).find(
        (o) => o.value === String(stats.retention_days),
      );
      if (opt) retentionEl.value = opt.value;
    }
  } catch {
    storeStatsEl.textContent = "Encrypted log · local only";
  }
}

async function tick() {
  try {
    const snapshot = await invoke("get_snapshot");
    render(snapshot);
    if (snapshot.connected) {
      await loadAllowlist();
      await refreshStats();
    }
  } catch (error) {
    render({
      connected: false,
      mode: null,
      browser_shield: false,
      system_shield: false,
      events: [],
      error: String(error),
    });
  }
}

async function persistSetup() {
  try {
    await invoke("save_setup_state", { state: setupState });
  } catch {
  }
}

async function finishSetupCompletely() {
  try {
    setupState = await invoke("mark_setup_complete");
  } catch {
    setupState.completed = true;
    setupState.welcome_done = true;
    setupState.host_done = true;
    await persistSetup();
  }
  closeWizard();
}

function showWizardStep() {
  const step = WIZARD_STEPS[wizardIndex];
  wizardProgress.textContent = `Step ${wizardIndex + 1} of ${WIZARD_STEPS.length}`;
  wizardTitle.textContent = step.title;
  wizardBody.textContent = step.body;
  wizardPrimary.textContent = step.primary;
  wizardPrimary.disabled = false;
  wizardStatus.hidden = true;
  wizardStatus.textContent = "";
  if (step.secondary) {
    wizardSecondary.hidden = false;
    wizardSecondary.textContent = step.secondary;
  } else {
    wizardSecondary.hidden = true;
  }
  wizard.hidden = false;
}

function closeWizard() {
  wizard.hidden = true;
  wizardBusy = false;
}

async function runWizardAction(step) {
  wizardBusy = true;
  wizardPrimary.disabled = true;
  wizardSecondary.disabled = true;
  wizardStatus.hidden = false;
  wizardStatus.textContent = "Working…";
  wizardStatus.dataset.state = "busy";

  const timeoutMs = 45000;
  const withTimeout = (p) =>
    Promise.race([
      p,
      new Promise((_, rej) => setTimeout(() => rej(new Error("Timed out — tap Skip or Finish setup")), timeoutMs)),
    ]);

  try {
    if (step.action === "ensure_host") {
      const result = await withTimeout(invoke("ensure_host"));
      setupState.host_done = true;
      wizardStatus.textContent = result.message || "Core is running.";
    } else if (step.action === "install_ca") {
      const msg = await withTimeout(invoke("install_local_ca"));
      setupState.ca_done = true;
      wizardStatus.textContent = msg;
    } else if (step.action === "open_extension") {
      const path = await withTimeout(invoke("open_extension_folder"));
      setupState.extension_done = true;
      wizardStatus.textContent = `Opened ${path}`;
    } else if (step.action === "system_shield") {
      await withTimeout(invoke("set_system_shield", { enabled: true }));
      try {
        wizardStatus.textContent = await invoke("set_system_proxy", { enabled: true });
      } catch (proxyErr) {
        wizardStatus.textContent = `Shield on. Proxy: ${proxyErr}`;
      }
      setupState.system_shield_done = true;
    } else if (step.action === "open_views") {
      setupState.views_done = true;
      await setExploreGroup("host", { persist: true, markViews: true });
      closeWizard();
      activateTab("explore");
      wizardStatus.textContent = "Opened Explore by host — tap Category anytime.";
      wizardStatus.dataset.state = "ok";
      await persistSetup();
      await tick();
      return;
    }
    if (step.flag) setupState[step.flag] = true;
    wizardStatus.dataset.state = "ok";
    await persistSetup();
    await tick();
    await new Promise((r) => setTimeout(r, 400));
    advanceWizard();
  } catch (error) {
    wizardStatus.textContent = String(error);
    wizardStatus.dataset.state = "err";
    wizardPrimary.disabled = false;
    wizardPrimary.textContent = "Retry";
  } finally {
    wizardBusy = false;
    wizardSecondary.disabled = false;
  }
}

function advanceWizard() {
  wizardIndex += 1;
  if (wizardIndex >= WIZARD_STEPS.length) {
    finishSetupCompletely();
    return;
  }
  if (WIZARD_STEPS[wizardIndex].id === "done") {
    setupState.completed = true;
    persistSetup();
  }
  showWizardStep();
}

async function onWizardPrimary() {
  if (wizardBusy) return;
  const step = WIZARD_STEPS[wizardIndex];
  if (step.id === "welcome") {
    setupState.welcome_done = true;
    await persistSetup();
    advanceWizard();
    return;
  }
  if (step.id === "done") {
    await finishSetupCompletely();
    return;
  }
  if (step.action) {
    await runWizardAction(step);
    return;
  }
  advanceWizard();
}

async function onWizardSecondary() {
  if (wizardBusy) return;
  const step = WIZARD_STEPS[wizardIndex];
  if (step.flag) setupState[step.flag] = true;
  await persistSetup();
  advanceWizard();
}

function resumeWizardIndex() {
  if (setupState.completed) return -1;
  if (!setupState.welcome_done) return 0;
  if (!setupState.host_done) return 1;
  if (!setupState.ca_done) return 2;
  if (!setupState.extension_done) return 3;
  if (!setupState.system_shield_done) return 4;
  if (!setupState.views_done) return 5;
  return 6;
}

async function startWizard(force = false) {
  try {
    setupState = normalizePrefs((await invoke("get_setup_state")) ?? setupState);
    applyPrefsToControls();
  } catch {
  }
  if (!force && setupState.completed) {
    closeWizard();
    return;
  }
  if (force) {
    try {
      setupState = normalizePrefs(await invoke("reset_setup"));
    } catch {
      setupState = normalizePrefs({
        schema_version: 2,
        welcome_done: false,
        host_done: false,
        ca_done: false,
        extension_done: false,
        system_shield_done: false,
        views_done: false,
        completed: false,
        explore_group_by: "host",
        explore_sort: "count",
        activity_sort: "newest",
      });
      await persistSetup();
    }
    applyPrefsToControls();
    wizardIndex = 0;
  } else {
    wizardIndex = resumeWizardIndex();
    if (wizardIndex < 0) {
      closeWizard();
      return;
    }
  }
  showWizardStep();
}

async function enableShield(withProxy) {
  await invoke("set_system_shield", { enabled: true });
  if (withProxy) {
    try {
      await invoke("set_system_proxy", { enabled: true });
    } catch (e) {
      showError(`Shield on. Proxy: ${e}`);
    }
  }
  await tick();
}

async function disableShield(withProxy) {
  await invoke("set_system_shield", { enabled: false });
  if (withProxy) {
    try {
      await invoke("set_system_proxy", { enabled: false });
    } catch {
    }
  }
  await tick();
}

function activateTab(name) {
  const tab = document.querySelector(`.tab[data-tab="${name}"]`);
  if (!tab) return;
  tab.classList.add("just-clicked");
  setTimeout(() => tab.classList.remove("just-clicked"), 180);
  document.querySelectorAll(".tab").forEach((t) => t.classList.remove("active"));
  document.querySelectorAll(".panel").forEach((p) => {
    const on = p.id === `panel-${name}`;
    p.classList.toggle("active", on);
    p.hidden = !on;
  });
  tab.classList.add("active");
  if (name === "settings") refreshStats();
  if (name === "explore") renderExplore();
}

document.querySelectorAll(".tab").forEach((tab) => {
  tab.addEventListener("click", () => {
    activateTab(tab.dataset.tab);
  });
});

document.addEventListener("keydown", (e) => {
  if (e.metaKey || e.ctrlKey || e.altKey || e.target.matches("textarea, input, select")) return;
  const map = {
    "1": "control",
    "2": "activity",
    "3": "explore",
    "4": "setup",
    "5": "settings",
  };
  if (map[e.key]) activateTab(map[e.key]);
});

document.querySelectorAll(".modes button").forEach((btn) => {
  btn.addEventListener("click", async () => {
    try {
      await invoke("set_mode", { mode: btn.dataset.mode });
      await tick();
    } catch (error) {
      showError(String(error));
    }
  });
});

document.getElementById("save-allowlist").addEventListener("click", async () => {
  try {
    await invoke("set_allowlist", { hostsCsv: allowlistEl.value });
    errorEl.hidden = true;
  } catch (error) {
    showError(String(error));
  }
});

systemShieldEl.addEventListener("change", async () => {
  if (suppressShieldEvent) return;
  if (systemShieldEl.checked) {
    systemShieldEl.checked = false;
    shieldModal.hidden = false;
    return;
  }
  try {
    await disableShield(true);
  } catch (error) {
    showError(String(error));
  }
});

document.getElementById("btn-shield-on").addEventListener("click", () => {
  shieldModal.hidden = false;
});
document.getElementById("btn-shield-off").addEventListener("click", async () => {
  try {
    await disableShield(true);
  } catch (e) {
    showError(String(e));
  }
});

document.getElementById("shield-cancel").addEventListener("click", () => {
  shieldModal.hidden = true;
});
document.getElementById("shield-close").addEventListener("click", () => {
  shieldModal.hidden = true;
});
document.getElementById("shield-confirm").addEventListener("click", async () => {
  shieldModal.hidden = true;
  try {
    await enableShield(true);
  } catch (error) {
    showError(String(error));
  }
});

async function doClearEvents() {
  try {
    await invoke("clear_events");
    await tick();
    await refreshStats();
    errorEl.hidden = true;
  } catch (e) {
    showError(String(e));
  }
}

if (retentionEl) {
  retentionEl.addEventListener("change", async () => {
    try {
      const days = Number(retentionEl.value);
      const stats = await invoke("set_retention_days", { days });
      storeStatsEl.textContent = `${stats.event_count} encrypted events · ${stats.retention_days}-day auto-prune · local only`;
      errorEl.hidden = true;
    } catch (e) {
      showError(String(e));
    }
  });
}

document.getElementById("clear-activity").addEventListener("click", doClearEvents);
document.getElementById("btn-clear-events").addEventListener("click", doClearEvents);

function onFeedControlsChanged() {
  renderFeed();
}
feedSearchEl?.addEventListener("input", onFeedControlsChanged);
feedFilterDecisionEl?.addEventListener("change", onFeedControlsChanged);
feedFilterActionEl?.addEventListener("change", onFeedControlsChanged);
feedFilterBucketEl?.addEventListener("change", onFeedControlsChanged);
feedSortEl?.addEventListener("change", async () => {
  onFeedControlsChanged();
  if (prefsReady) {
    await saveViewPrefs({ activity_sort: feedSortEl.value });
  }
});

document.querySelectorAll(".explore-group-toggle .chip, .explore-dual .dual-card").forEach((el) => {
  el.addEventListener("click", async () => {
    await setExploreGroup(el.dataset.group || "host", { persist: true, markViews: true });
  });
});
exploreSortEl?.addEventListener("change", async () => {
  exploreActiveGroup = null;
  if (prefsReady) {
    await saveViewPrefs({ explore_sort: exploreSortEl.value });
  }
  renderExplore();
});
document.getElementById("explore-back")?.addEventListener("click", () => {
  exploreActiveGroup = null;
  renderExplore();
});
prefExploreGroupEl?.addEventListener("change", async () => {
  await setExploreGroup(prefExploreGroupEl.value, { persist: true });
});
prefExploreSortEl?.addEventListener("change", async () => {
  if (exploreSortEl) exploreSortEl.value = prefExploreSortEl.value;
  await saveViewPrefs({ explore_sort: prefExploreSortEl.value });
  exploreActiveGroup = null;
  renderExplore();
});
prefActivitySortEl?.addEventListener("change", async () => {
  if (feedSortEl) feedSortEl.value = prefActivitySortEl.value;
  await saveViewPrefs({ activity_sort: prefActivitySortEl.value });
  renderFeed();
});
document.getElementById("btn-open-explore-host")?.addEventListener("click", async () => {
  await setExploreGroup("host", { persist: true, markViews: true });
  activateTab("explore");
});
document.getElementById("btn-open-explore-cat")?.addEventListener("click", async () => {
  await setExploreGroup("bucket", { persist: true, markViews: true });
  activateTab("explore");
});
document.getElementById("event-close")?.addEventListener("click", closeEventDetail);
document.getElementById("event-close-btn")?.addEventListener("click", closeEventDetail);
document.getElementById("event-allowlist")?.addEventListener("click", allowlistSelectedHost);
eventModal?.addEventListener("click", (e) => {
  if (e.target === eventModal) closeEventDetail();
});

document.getElementById("btn-open-data").addEventListener("click", async () => {
  try {
    await invoke("open_data_dir");
  } catch (e) {
    showError(String(e));
  }
});
document.getElementById("btn-proxy-on").addEventListener("click", async () => {
  try {
    await enableShield(true);
  } catch (e) {
    showError(String(e));
  }
});
document.getElementById("btn-proxy-off").addEventListener("click", async () => {
  try {
    await disableShield(true);
  } catch (e) {
    showError(String(e));
  }
});
document.getElementById("btn-restart-host").addEventListener("click", async () => {
  try {
    const r = await invoke("ensure_host");
    await tick();
    errorEl.hidden = true;
    storeStatsEl.textContent = r.message || "Core running";
  } catch (e) {
    showError(String(e));
  }
});
document.getElementById("btn-reset-setup").addEventListener("click", async () => {
  await startWizard(true);
});

document.querySelectorAll(".info").forEach((btn) => {
  btn.addEventListener("click", () => showTip(btn.dataset.tip));
});
document.getElementById("tip-close").addEventListener("click", hideTip);
tip.addEventListener("click", (e) => {
  if (e.target === tip) hideTip();
});
shieldModal.addEventListener("click", (e) => {
  if (e.target === shieldModal) shieldModal.hidden = true;
});

wizardPrimary.addEventListener("click", onWizardPrimary);
wizardSecondary.addEventListener("click", onWizardSecondary);
document.getElementById("wizard-finish").addEventListener("click", () => finishSetupCompletely());
document.getElementById("rerun-setup").addEventListener("click", () => startWizard(true));

(async () => {
  try {
    await invoke("ensure_host");
  } catch {
  }
  try {
    setupState = normalizePrefs((await invoke("get_setup_state")) ?? setupState);
    applyPrefsToControls();
  } catch {
    applyPrefsToControls();
  }
  prefsReady = true;
  if (tauriEvent?.listen) {
    try {
      await tauriEvent.listen("navigate-tab", async (event) => {
        const payload = event.payload;
        if (typeof payload === "string") {
          activateTab(payload);
          return;
        }
        if (payload && typeof payload === "object") {
          if (payload.group) {
            await setExploreGroup(payload.group, { persist: true });
          }
          activateTab(payload.tab || "control");
        }
      });
      await tauriEvent.listen("tray-refresh", () => {
        tick();
      });
    } catch {
    }
  }
  await startWizard(false);
  await tick();
  setInterval(tick, 1000);
})();
