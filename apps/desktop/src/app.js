const { invoke } = window.__TAURI__.core;

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
    body: "Redacted cookie Set/Send events with decisions. Values never leave this device. Auto-pruned per Settings retention.",
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

let allowlistLoaded = false;
let suppressShieldEvent = false;
let wizardIndex = 0;
let wizardBusy = false;
let setupState = {
  schema_version: 1,
  welcome_done: false,
  host_done: false,
  ca_done: false,
  extension_done: false,
  system_shield_done: false,
  completed: false,
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

  const events = snapshot.events ?? [];
  feedEmpty.hidden = events.length > 0;
  feedEl.replaceChildren(
    ...events.map((e) => {
      const li = document.createElement("li");
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
      return li;
    }),
  );
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
  return 5;
}

async function startWizard(force = false) {
  try {
    setupState = (await invoke("get_setup_state")) ?? setupState;
  } catch {
  }
  if (!force && setupState.completed) {
    closeWizard();
    return;
  }
  if (force) {
    try {
      setupState = await invoke("reset_setup");
    } catch {
      setupState = {
        schema_version: 1,
        welcome_done: false,
        host_done: false,
        ca_done: false,
        extension_done: false,
        system_shield_done: false,
        completed: false,
      };
      await persistSetup();
    }
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
}

document.querySelectorAll(".tab").forEach((tab) => {
  tab.addEventListener("click", () => {
    activateTab(tab.dataset.tab);
  });
});

document.addEventListener("keydown", (e) => {
  if (e.metaKey || e.ctrlKey || e.altKey || e.target.matches("textarea, input, select")) return;
  const map = { "1": "control", "2": "activity", "3": "setup", "4": "settings" };
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
  await startWizard(false);
  await tick();
  setInterval(tick, 1000);
})();
