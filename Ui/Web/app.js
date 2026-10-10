const query = new URLSearchParams(window.location.search);
const mockBridge = query.has("mock");
const t = (key, ...args) => window.I18n.t(key, ...args);
const pending = new Map();
const elements = {};
const state = { settings: {}, sources: [], versions: [], selected: new Set(), logs: [], running: false, paused: false, progress: null };
let sequence = 0;
let searchTimer = 0;
let language = "zh-cn";
let theme = "system";

function refreshIcons() {
  if (!window.lucide) return;
  window.lucide.createIcons({ attrs: { "stroke-width": 1.8, "aria-hidden": "true" } });
}

function call(action, payload = {}) {
  if (mockBridge) return window.__mockBridge(action, payload);
  if (!window.chrome?.webview) return Promise.reject(new Error("Bridge unavailable"));
  sequence += 1;
  const id = sequence;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    window.chrome.webview.postMessage({ id, action, payload });
    window.setTimeout(() => {
      if (!pending.has(id)) return;
      pending.delete(id);
      reject(new Error(t("msg.failed")));
    }, 180000);
  });
}

function errorMessage(error) {
  const raw = error?.message || "msg.failed";
  const keys = {
    "directory.required": "error.directoryRequired",
    "url.blocked": "error.urlBlocked",
    "url.invalid": "error.urlInvalid",
    "name.required": "error.nameRequired",
    "source.missing": "error.sourceMissing",
    "download.noSelection": "error.downloadNoSelection",
    "download.notRunning": "error.downloadNotRunning",
    "operation.running": "error.operationRunning",
    "network.failed": "error.networkFailed"
  };
  return keys[raw] ? t(keys[raw]) : /^[a-z]+\.[a-zA-Z]+$/.test(raw) ? t("msg.failed") : raw;
}

function showMessage(text, tone = "") {
  elements.toastText.textContent = text;
  elements.toastIcon.textContent = tone === "bad" ? "!" : "✓";
  elements.toast.classList.add("visible");
  window.clearTimeout(showMessage.timer);
  showMessage.timer = window.setTimeout(() => elements.toast.classList.remove("visible"), 2800);
}

function applyLanguage(value) {
  language = value === "en-us" ? "en-us" : "zh-cn";
  window.I18n.apply(language);
  render();
  refreshIcons();
}

function applyTheme(value) {
  theme = value === "dark" ? "dark" : value === "light" ? "light" : "system";
  const dark = theme === "dark" || theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches;
  document.documentElement.classList.toggle("dark", dark);
  document.documentElement.classList.toggle("light", !dark);
  refreshIcons();
}

function setRunning(running) {
  state.running = running;
  elements.refreshButton.disabled = running; elements.headerRefreshButton.disabled = running;
  elements.startButton.disabled = running; elements.headerStartButton.disabled = running;
  elements.pauseButton.disabled = !running;
  elements.stopButton.disabled = !running;
  elements.selectAllButton.disabled = running;
  elements.clearSelectionButton.disabled = running;
  elements.progressTrack.classList.toggle("visible", running);
}

function filteredVersions() {
  const needle = elements.searchInput.value.trim().toLowerCase();
  return state.versions.filter(item => !needle || item.id.toLowerCase().includes(needle));
}

function renderVersions(animate = true) {
  const versions = filteredVersions();
  elements.versionCount.textContent = `${versions.length} / ${state.versions.length}`;
  elements.footerVersions.textContent = `${versions.length} / ${state.versions.length}`;
  elements.emptyState.hidden = versions.length > 0;
  elements.emptyTitle.textContent = state.versions.length ? t("list.noMatch") : t("list.empty");
  elements.emptyCopy.textContent = state.versions.length ? t("list.noMatchCopy") : t("list.emptyCopy");
  elements.versionList.replaceChildren(...versions.map((item, index) => {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "site-row" + (state.selected.has(item.id) ? " selected" : "") + (item.isDownloaded ? " unconfigured" : "") + (animate ? " animate-in" : "");
    row.dataset.id = item.id;
    row.style.setProperty("--stagger", `${Math.min(index, 16) * 35}ms`);
    row.innerHTML = `<span class="version-dot ${item.isDownloaded ? "dot-good" : "dot-idle"}"></span><span class="site-copy"><strong class="site-name">${escapeHtml(item.id)}</strong><small class="site-host">${escapeHtml(item.releaseTime ? new Date(item.releaseTime).toLocaleDateString() : "")}</small></span><span class="status-pill ${item.isDownloaded ? "tone-good" : "tone-idle"}">${item.isDownloaded ? t("downloaded") : t("notDownloaded")}</span>`;
    row.addEventListener("click", () => toggleSelection(item.id));
    return row;
  }));
  elements.selectionCount.textContent = `${state.selected.size} ${t("selected")}`;
  elements.downloadPath.textContent = state.settings.downloadDirectory || "";
  elements.footerSource.textContent = state.settings.mirror === "bmclapi" ? t("mirror.bmclapi") : state.settings.mirror === "custom" ? t("mirror.custom") : t("mirror.official");
  elements.footerConfig.textContent = state.configExists ? "" : t("footer.config");
  refreshIcons();
}

function render() {
  elements.headerSummary.textContent = state.versions.length ? `${state.versions.length} ${t("list.versions")}` : t("app.ready");
  elements.settingDirectory.value = state.settings.downloadDirectory || "";
  elements.settingMirror.value = state.settings.mirror || "official";
  elements.settingProxy.value = state.settings.proxy || "";
  elements.settingConcurrency.value = state.settings.maxConcurrent || 3;
  elements.settingTimeout.value = state.settings.requestTimeoutSeconds || 30;
  elements.settingRetry.value = state.settings.retryCount || 2;
  elements.settingLanguage.value = language;
  elements.settingTheme.value = theme;
  elements.settingSource.replaceChildren(...state.sources.map(source => {
    const option = document.createElement("option");
    option.value = source.id;
    option.textContent = source.name;
    return option;
  }));
  elements.settingSource.value = state.settings.customSourceId || "";
  elements.settingSource.classList.toggle("hidden", state.settings.mirror !== "custom" || state.sources.length === 0);
  renderVersions(false);
  renderLogs();
  refreshIcons();
}

function renderLogs() {
  elements.logList.textContent = state.logs.slice(-300).join("\n");
  elements.logList.scrollTop = elements.logList.scrollHeight;
}

function toggleSelection(id) {
  if (state.running) return;
  if (state.selected.has(id)) state.selected.delete(id); else state.selected.add(id);
  renderVersions(false);
}

function escapeHtml(value) {
  return String(value ?? "").replace(/[&<>"']/g, char => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[char]));
}

async function loadState() {
  const result = await call("getState");
  applyState(result);
}

function applyState(result) {
  state.configExists = Boolean(result.configExists);
  state.settings = result.settings || {};
  state.sources = result.sources || [];
  state.versions = result.versions || [];
  const detectedLanguage = state.settings.language || ((navigator.language || "").toLowerCase().startsWith("zh") ? "zh-cn" : "en-us");
  language = detectedLanguage;
  theme = state.settings.theme || "system";
  applyLanguage(language);
  applyTheme(theme);
}

async function refreshVersions() {
  if (state.running) return;
  setRunning(true);
  elements.statusText.textContent = t("status.refreshing");
  try {
    const result = await call("refreshVersions", { settings: collectSettings() });
    applyState(result);
    showMessage(t("app.ready"));
  } catch (error) {
    showMessage(errorMessage(error), "bad");
  } finally {
    setRunning(false);
    elements.statusText.textContent = t("status.ready");
  }
}

function collectSettings() {
  return {
    downloadDirectory: elements.settingDirectory.value,
    mirror: elements.settingMirror.value,
    customSourceId: elements.settingSource.value,
    proxy: elements.settingProxy.value,
    maxConcurrent: Number(elements.settingConcurrency.value),
    requestTimeoutSeconds: Number(elements.settingTimeout.value),
    retryCount: Number(elements.settingRetry.value),
    language,
    theme
  };
}

function openOverlay(id) {
  document.getElementById(id).classList.add("open");
  refreshIcons();
}

function closeOverlay(id) {
  document.getElementById(id).classList.remove("open");
}

function handleEvent(message) {
  if (message?.type === "status") {
    elements.statusText.textContent = t(message.key || "status.ready");
    return;
  }
  if (message?.type === "log") {
    const name = message.key?.split(":")[0] || "";
    const arg = message.key?.includes(":") ? message.key.split(":").slice(1).join(":") : "";
    const text = t(name, arg);
    state.logs.push(`${new Date().toLocaleTimeString()}  ${text}`);
    renderLogs();
    return;
  }
  if (message?.type === "progress") {
    state.progress = message.value || null;
    const progress = state.progress;
    const overall = progress?.total ? progress.completed / progress.total : 0;
    const bytes = progress?.totalBytes ? progress.doneBytes / progress.totalBytes : 0;
    elements.overallFill.style.transform = `scaleX(${Math.min(1, overall)})`;
    elements.bytesFill.style.transform = `scaleX(${Math.min(1, bytes)})`;
    elements.overallText.textContent = `${progress?.completed || 0} / ${progress?.total || 0}`;
    elements.bytesText.textContent = `${Math.round(Math.min(1, bytes) * 100)}%`;
    elements.speedText.textContent = `${t("speed")} ${formatBytes(progress?.bytesPerSecond || 0)}  ${t("remaining")} ${formatSeconds(progress?.remainingSeconds || 0)}`;
  }
}

function formatBytes(value) {
  if (value > 1024 * 1024) return `${(value / 1024 / 1024).toFixed(1)} MB/s`;
  if (value > 1024) return `${(value / 1024).toFixed(1)} KB/s`;
  return `${Math.round(value)} B/s`;
}

function formatSeconds(value) {
  if (!value || value <= 0) return "--";
  return `${Math.floor(value / 60)}m ${Math.round(value % 60)}s`;
}

function handleMessage(event) {
  const message = event.data;
  if (message?.type === "status" || message?.type === "log" || message?.type === "progress") return handleEvent(message);
  if (!message?.id || !pending.has(message.id)) return;
  const resolver = pending.get(message.id);
  pending.delete(message.id);
  if (message.ok) resolver.resolve(message.result); else resolver.reject(new Error(message.error || t("msg.failed")));
}

async function startDownload() {
  if (state.running) return;
  const selectedIds = [...state.selected];
  if (!selectedIds.length) { showMessage(t("msg.noSelection"), "bad"); return; }
  setRunning(true);
  try {
    const result = await call("startDownload", { settings: collectSettings(), selectedIds });
    showMessage(result?.outcome === "Completed" ? t("log.allDone") : t("log.stopped"));
    const refreshed = await call("getState");
    applyState(refreshed);
  } catch (error) {
    showMessage(errorMessage(error), "bad");
  } finally {
    setRunning(false);
    state.paused = false;
  }
}

async function importSource() {
  try {
    const result = await call("importSource", {
      name: elements.importName.value,
      address: elements.importAddress.value
    });
    applyState(result);
    elements.importName.value = "";
    elements.importAddress.value = "";
    closeOverlay("importOverlay");
    showMessage(t("msg.imported"));
  } catch (error) {
    elements.importStatus.hidden = false;
    elements.importStatus.className = "form-status bad";
    elements.importStatus.textContent = errorMessage(error);
  }
}

function bindEvents() {
  elements.languageButton.addEventListener("click", () => applyLanguage(language === "zh-cn" ? "en-us" : "zh-cn"));
  elements.themeButton.addEventListener("click", () => applyTheme(theme === "dark" ? "light" : "dark"));
  elements.settingsButton.addEventListener("click", () => { render(); openOverlay("settingsOverlay"); });
  elements.importButton.addEventListener("click", () => openOverlay("importOverlay"));
  elements.refreshButton.addEventListener("click", refreshVersions);
  elements.headerRefreshButton.addEventListener("click", refreshVersions);
  elements.startButton.addEventListener("click", startDownload);
  elements.headerStartButton.addEventListener("click", startDownload);
  elements.pauseButton.addEventListener("click", async () => {
    try { await call(state.paused ? "resumeDownload" : "pauseDownload"); state.paused = !state.paused; elements.pauseButton.querySelector("span").textContent = state.paused ? t("btn.resume") : t("btn.pause"); }
    catch (error) { showMessage(errorMessage(error), "bad"); }
  });
  elements.stopButton.addEventListener("click", async () => { try { await call("stopDownload"); } catch (error) { showMessage(errorMessage(error), "bad"); } });
  elements.openDataButton.addEventListener("click", async () => { try { await call("openDataDirectory"); } catch (error) { showMessage(errorMessage(error), "bad"); } });
  elements.selectAllButton.addEventListener("click", () => { state.selected = new Set(state.versions.filter(item => !item.isDownloaded).map(item => item.id)); renderVersions(false); });
  elements.clearSelectionButton.addEventListener("click", () => { state.selected.clear(); renderVersions(false); });
  elements.clearLogButton.addEventListener("click", () => { state.logs = []; renderLogs(); });
  elements.searchInput.addEventListener("input", () => { elements.clearSearchButton.hidden = !elements.searchInput.value; window.clearTimeout(searchTimer); searchTimer = window.setTimeout(() => renderVersions(true), 90); });
  elements.clearSearchButton.addEventListener("click", () => { elements.searchInput.value = ""; elements.clearSearchButton.hidden = true; renderVersions(true); });
  elements.saveSettingsButton.addEventListener("click", async () => {
    try { const result = await call("saveSettings", { settings: collectSettings() }); applyState(result); closeOverlay("settingsOverlay"); showMessage(t("msg.saved")); }
    catch (error) { elements.settingsStatus.textContent = errorMessage(error); }
  });
  elements.chooseDirectoryButton.addEventListener("click", async () => { try { const path = await call("selectDirectory"); if (path) elements.settingDirectory.value = path; } catch (error) { showMessage(errorMessage(error), "bad"); } });
  elements.importConfirmButton.addEventListener("click", importSource);
  document.querySelectorAll("[data-close]").forEach(button => button.addEventListener("click", () => closeOverlay(button.dataset.close)));
  document.querySelectorAll("#mirrorGroup .segment").forEach(button => button.addEventListener("click", () => {
    document.querySelectorAll("#mirrorGroup .segment").forEach(item => item.classList.remove("active"));
    button.classList.add("active");
    elements.settingMirror.value = button.dataset.mirror;
  }));
  window.addEventListener("keydown", event => {
    if (event.key === "Escape") document.querySelectorAll(".overlay.open").forEach(overlay => overlay.classList.remove("open"));
  });
  if (window.chrome?.webview) window.chrome.webview.addEventListener("message", handleMessage);
}

function cacheElements() {
  ["toast", "toastIcon", "toastText", "headerSummary", "headerRefreshButton", "headerStartButton", "languageButton", "themeButton", "settingsButton", "importButton", "progressTrack", "progressFill", "searchInput", "clearSearchButton", "versionCount", "versionList", "emptyState", "emptyTitle", "emptyCopy", "refreshButton", "selectAllButton", "clearSelectionButton", "selectionCount", "downloadPath", "overallFill", "overallText", "bytesFill", "bytesText", "speedText", "statusText", "startButton", "pauseButton", "stopButton", "openDataButton", "clearLogButton", "logList", "footerVersions", "footerSource", "footerConfig", "settingsOverlay", "settingDirectory", "chooseDirectoryButton", "settingMirror", "settingSource", "settingProxy", "settingConcurrency", "settingTimeout", "settingRetry", "settingLanguage", "settingTheme", "settingsStatus", "saveSettingsButton", "importOverlay", "importName", "importAddress", "importStatus", "importConfirmButton"].forEach(id => { elements[id] = document.getElementById(id); });
}

async function init() {
  cacheElements();
  bindEvents();
  applyLanguage("zh-cn");
  applyTheme("system");
  try {
    await loadState();
    if (!mockBridge) await refreshVersions();
  } catch (error) {
    showMessage(errorMessage(error), "bad");
  }
  refreshIcons();
}

window.addEventListener("DOMContentLoaded", init);
window.appState = state;
window.renderVersions = renderVersions;







