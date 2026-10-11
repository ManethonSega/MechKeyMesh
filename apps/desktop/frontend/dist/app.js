const $ = (selector) => document.querySelector(selector);
const invoke = (command, args = {}) => window.__TAURI__.core.invoke(command, args);
const SVG_NS = "http://www.w3.org/2000/svg";

let activeLayout = null;
let activeDriverId = null;
let lightingEffects = [];
const selectedKeyIds = new Set();

const pages = {
  home: ["Home", "Device overview and connection health"],
  keys: ["Keys", "Data-driven physical layout and universal key actions"],
  macros: ["Macros", "Create, edit, persist and safely simulate universal macros"],
  lighting: ["Lighting", "Onboard effects and persistent per-key RGB"],
  diagnostics: ["Diagnostics", "Connection, command and recovery log"]
};

function showPage(name) {
  document.querySelectorAll(".page").forEach((page) => page.classList.remove("active"));
  document.querySelectorAll("#nav button").forEach((button) => {
    button.classList.toggle("active", button.dataset.page === name);
  });
  document.querySelector("#page-" + name)?.classList.add("active");
  $("#pageTitle").textContent = pages[name]?.[0] || name;
  $("#pageSubtitle").textContent = pages[name]?.[1] || "";

  if (name === "diagnostics") refreshLogs();
  if (name === "keys" && activeLayout) renderKeyboard(activeLayout);
}

document.querySelectorAll("#nav button:not(:disabled)").forEach((button) => {
  button.addEventListener("click", () => showPage(button.dataset.page));
});

function initialTheme() {
  const stored = localStorage.getItem("mechkeymesh-theme");
  if (stored === "light" || stored === "dark") return stored;
  return window.matchMedia?.("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

function applyTheme(theme) {
  document.documentElement.dataset.theme = theme;
  localStorage.setItem("mechkeymesh-theme", theme);
  $("#themeToggle").textContent = theme === "dark" ? "Light mode" : "Dark mode";
  document.querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", theme === "dark" ? "#081327" : "#EDF2F7");
}

$("#themeToggle").addEventListener("click", () => {
  const next = document.documentElement.dataset.theme === "dark" ? "light" : "dark";
  applyTheme(next);
});

$("#brightness").addEventListener("input", (event) => {
  $("#brightnessOut").value = event.target.value;
});
$("#speed").addEventListener("input", (event) => {
  $("#speedOut").value = event.target.value;
});
$("#lightingMode").addEventListener("change", updateLightingControls);

async function loadLightingEffects() {
  lightingEffects = await invoke("get_lighting_effects");
  $("#lightingMode").innerHTML = lightingEffects
    .map((effect) => `<option value="${effect.mode}">${effect.mode}. ${effect.name}</option>`)
    .join("");
  updateLightingControls();
}

function currentLightingEffect() {
  const mode = Number($("#lightingMode").value);
  return lightingEffects.find((effect) => effect.mode === mode) || null;
}

function setControlVisible(selector, visible) {
  document.querySelector(selector)?.classList.toggle("hidden-control", !visible);
}

function updateLightingControls() {
  const effect = currentLightingEffect();
  if (!effect) return;

  const hasColor = effect.color === "color-or-rainbow";
  const rainbowOnly = effect.color === "rainbow-only";

  setControlVisible("#colorControl", hasColor);
  setControlVisible("#colorfulControl", hasColor);
  setControlVisible("#rainbowOnlyControl", rainbowOnly);
  setControlVisible("#brightnessControl", effect.brightness);
  setControlVisible("#speedControl", effect.speed);
  setControlVisible("#directionControl", effect.direction != null);

  if (effect.direction === "left-right") {
    $("#direction").innerHTML =
      '<option value="0">Left → Right</option><option value="1">Right → Left</option>';
  } else if (effect.direction === "up-down") {
    $("#direction").innerHTML =
      '<option value="0">Up → Down</option><option value="1">Down → Up</option>';
  }

  const params = [];
  if (hasColor) params.push("color", "rainbow");
  if (rainbowOnly) params.push("built-in rainbow");
  if (effect.brightness) params.push("brightness");
  if (effect.speed) params.push("speed");
  if (effect.direction === "left-right") params.push("left/right direction");
  if (effect.direction === "up-down") params.push("up/down direction");

  $("#lightingEffectTitle").textContent = `Mode ${effect.mode} · ${effect.name}`;
  $("#lightingEffectParams").textContent =
    params.length ? "Controls: " + params.join(", ") : "No adjustable parameters.";
}

function stateClass(state) {
  if (["ready"].includes(state)) return "ready";
  if (["busy", "detecting", "opening", "initializing"].includes(state)) return "busy";
  if (["recovery", "ambiguous"].includes(state)) return "recovery";
  if (state === "unsupported") return "unsupported";
  return "disconnected";
}

function updateStatus(status) {
  const state = status?.state || "disconnected";
  const device = status?.device || null;
  const nextDriverId = device?.driverId || null;
  const pill = $("#connectionPill");
  pill.textContent = state;
  pill.className = "pill " + stateClass(state);

  $("#deviceName").textContent = device?.name || "AULA F108 Pro";
  $("#deviceDetail").textContent =
    status?.lastError ||
    (state === "ready" ? "Configuration interface ready" : "Waiting for a supported keyboard...");
  $("#connectionType").textContent = device?.connection || "—";
  $("#driverId").textContent = device?.driverId || "aula.f108pro.usb";
  $("#stateText").textContent = state;
  $("#candidateCount").textContent = String(status?.candidateCount ?? 0);
  $("#reconnectCount").textContent = String(status?.reconnectCount ?? 0);
  $("#activeOperation").textContent = status?.activeOperation
    ? `Active operation #${status.activeOperation.id}: ${status.activeOperation.name}`
    : "No active hardware operation.";
  $("#vidpid").textContent =
    device?.vendorId != null
      ? device.vendorId.toString(16).padStart(4, "0").toUpperCase() +
        " / " +
        device.productId.toString(16).padStart(4, "0").toUpperCase()
      : "—";

  $("#applyLighting").disabled = state === "busy";
  $("#applyLighting").textContent =
    state === "ready" ? "Save & apply to keyboard" : "Save desired state";

  if (nextDriverId !== activeDriverId) {
    activeDriverId = nextDriverId;
    void loadPrimaryLayout();
  }
}

async function pollStatus() {
  try {
    updateStatus(await invoke("get_status"));
  } catch (error) {
    updateStatus({ state: "disconnected", lastError: String(error), device: null });
  }
}

function hexRgb(hex) {
  const value = parseInt(hex.slice(1), 16);
  return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
}

function rgbHex(r, g, b) {
  return "#" + [r, g, b]
    .map((value) => Number(value).toString(16).padStart(2, "0"))
    .join("");
}

async function loadPersistence() {
  const status = await invoke("get_persistence_status");
  $("#schemaVersion").textContent = "schema " + status.schemaVersion;
  $("#databaseIntegrity").textContent = status.integrityOk ? "OK" : "Check failed";
  $("#persistenceView").textContent =
    `SQLite | schema ${status.schemaVersion} | ${status.journalMode} | integrity ${status.integrityOk ? "OK" : "FAILED"}\n${status.path}`;
}

async function loadDesiredLighting() {
  const desired = await invoke("get_desired_lighting");
  if (!desired) return;

  const config = desired.config;
  $("#lightingMode").value = String(config.mode);
  $("#lightingColor").value = rgbHex(config.r, config.g, config.b);
  $("#brightness").value = String(config.brightness);
  $("#brightnessOut").value = String(config.brightness);
  $("#speed").value = String(config.speed);
  $("#speedOut").value = String(config.speed);
  $("#direction").value = String(config.direction);
  $("#colorful").checked = Boolean(config.colorful);
  $("#desiredProfile").textContent = desired.scope.profileId;
  updateLightingControls();
}

function layoutKindLabel(kind) {
  const labels = {
    "full-size": "Full-size",
    "compact1800": "1800 compact",
    "tkl": "TKL",
    "percent75": "75%",
    "percent65": "65%",
    "percent60": "60%",
    "percent40": "40%",
    "alice": "Alice",
    "split": "Split",
    "ortholinear": "Ortholinear",
    "custom": "Custom"
  };
  return labels[kind] || kind;
}

function actionLabel(action) {
  if (!action) return "Unknown";
  switch (action.type) {
    case "default": return "Default";
    case "disabled": return "Disabled";
    case "keyboard-key":
      return "Keyboard usage 0x" + Number(action.usage).toString(16).padStart(2, "0").toUpperCase();
    case "modified-key":
      return `${(action.modifiers || []).join(" + ")} + HID ${action.key}`;
    case "media": return "Media: " + action.action;
    case "mouse": return "Mouse: " + action.action;
    case "windows": return "Windows: " + action.action;
    case "open-application": return "Open application";
    case "open-file": return "Open file";
    case "open-url": return "Open URL";
    case "send-text": return "Send text";
    case "macro": return "Macro: " + action.macroId;
    case "layer": return `Layer ${action.action}: ${action.layerId}`;
    case "vendor-specific": return `Vendor: ${action.namespace}/${action.action}`;
    default: return action.type || "Unknown";
  }
}

function keyTitle(key) {
  const parts = [
    key.label,
    key.section ? `section ${key.section}` : null,
    key.hidUsage == null ? null : `HID 0x${Number(key.hidUsage).toString(16).padStart(2, "0").toUpperCase()}`,
    key.deviceKeyIndex == null ? null : `device index ${key.deviceKeyIndex}`,
    key.ledIndex == null ? null : `LED ${key.ledIndex}`
  ].filter(Boolean);
  return parts.join(" | ");
}

function keyFontSize(key) {
  const labelLength = Math.max(String(key.label || "").length, 1);
  const widthLimited = key.width / (labelLength * 0.72);
  const heightLimited = key.height * 0.32;
  return Math.max(5, Math.min(8, widthLimited, heightLimited));
}

function renderKeyboard(layout) {
  const layoutChanged = activeLayout?.id !== layout.id;
  if (layoutChanged) selectedKeyIds.clear();
  activeLayout = layout;
  window.mechKeyMeshActiveLayout = layout;

  $("#layoutName").textContent = layout.name;
  $("#layoutKind").textContent = layoutKindLabel(layout.kind);
  $("#layoutKeyCount").textContent = `${layout.keys.length} keys`;
  $("#layoutSummary").textContent =
    `${layout.id} · schema ${layout.schemaVersion} · ${layout.coordinateSpace.width} × ${layout.coordinateSpace.height}`;

  const viewport = $("#keyboardViewport");
  viewport.innerHTML = "";

  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("class", "keyboard-svg");
  svg.setAttribute(
    "viewBox",
    `0 0 ${layout.coordinateSpace.width} ${layout.coordinateSpace.height}`
  );
  svg.setAttribute("preserveAspectRatio", "xMidYMid meet");
  svg.setAttribute("role", "group");
  svg.setAttribute("aria-label", layout.name + " keyboard layout");

  for (const key of layout.keys) {
    const group = document.createElementNS(SVG_NS, "g");
    group.setAttribute("class", "keyboard-key");
    group.setAttribute("data-key-id", key.id);
    group.setAttribute("tabindex", "0");
    group.setAttribute("role", "button");
    group.setAttribute("aria-label", keyTitle(key));
    group.setAttribute("aria-pressed", "false");

    if (Number(key.rotation || 0) !== 0) {
      const pivot = key.rotationOrigin || {
        x: key.x + key.width / 2,
        y: key.y + key.height / 2
      };
      group.setAttribute("transform", `rotate(${key.rotation} ${pivot.x} ${pivot.y})`);
    }

    const title = document.createElementNS(SVG_NS, "title");
    title.textContent = keyTitle(key);

    const rect = document.createElementNS(SVG_NS, "rect");
    rect.setAttribute("x", key.x);
    rect.setAttribute("y", key.y);
    rect.setAttribute("width", key.width);
    rect.setAttribute("height", key.height);
    rect.setAttribute("rx", Math.min(4, key.width / 7, key.height / 7));

    const text = document.createElementNS(SVG_NS, "text");
    text.setAttribute("x", key.x + key.width / 2);
    text.setAttribute("y", key.y + key.height / 2 + 0.5);
    text.style.fontSize = `${keyFontSize(key)}px`;
    text.textContent = key.label;

    group.append(title, rect, text);
    group.addEventListener("click", (event) => selectKey(key, event.ctrlKey || event.metaKey));
    group.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        selectKey(key, event.ctrlKey || event.metaKey);
      }
    });
    svg.appendChild(group);
  }

  viewport.appendChild(svg);
  refreshKeySelection();
  window.dispatchEvent(new CustomEvent("mechkeymesh-layout-changed", { detail: { layout } }));
}

function selectKey(key, additive) {
  if (!additive) selectedKeyIds.clear();

  if (additive && selectedKeyIds.has(key.id)) {
    selectedKeyIds.delete(key.id);
  } else {
    selectedKeyIds.add(key.id);
  }

  refreshKeySelection();

  const selected = activeLayout.keys.filter((item) => selectedKeyIds.has(item.id));
  if (selected.length === 0) {
    clearKeyInspector();
  } else if (selected.length === 1) {
    showKeyInspector(selected[0]);
  } else {
    $("#selectedKeyName").textContent = `${selected.length} keys selected`;
    $("#selectedKeyDetails").innerHTML =
      `<div><dt>Selection</dt><dd>${selected.map((item) => item.label).join(", ")}</dd></div>
       <div><dt>Action editing</dt><dd>Use the action editor to apply one assignment to the full selection.</dd></div>`;
  }
}

function refreshKeySelection() {
  document.querySelectorAll(".keyboard-key").forEach((element) => {
    const selected = selectedKeyIds.has(element.dataset.keyId);
    element.classList.toggle("selected", selected);
    element.setAttribute("aria-pressed", String(selected));
  });
}

function clearKeyInspector() {
  $("#selectedKeyName").textContent = "Select a key";
  $("#selectedKeyDetails").innerHTML =
    `<div><dt>Physical position</dt><dd>—</dd></div>
     <div><dt>Electrical matrix</dt><dd>—</dd></div>
     <div><dt>Device key index</dt><dd>—</dd></div>
     <div><dt>LED index</dt><dd>—</dd></div>
     <div><dt>Default action</dt><dd>—</dd></div>`;
}

function showKeyInspector(key) {
  $("#selectedKeyName").textContent = key.label;

  const matrix = key.matrix
    ? `unit ${key.matrix.unit ?? 0}, row ${key.matrix.row}, column ${key.matrix.column}`
    : "Unverified";
  const grid = key.gridPosition
    ? `unit ${key.gridPosition.unit ?? 0}, row ${key.gridPosition.row}, column ${key.gridPosition.column}`
    : "—";
  const hid = key.hidUsage == null
    ? "—"
    : "0x" + Number(key.hidUsage).toString(16).padStart(2, "0").toUpperCase();
  const pivot = key.rotationOrigin
    ? `, pivot ${key.rotationOrigin.x}, ${key.rotationOrigin.y}`
    : "";

  $("#selectedKeyDetails").innerHTML =
    `<div><dt>Physical position</dt><dd>x ${key.x}, y ${key.y}, ${key.width} × ${key.height}, rotation ${key.rotation || 0}°${pivot}</dd></div>
     <div><dt>Section</dt><dd>${key.section || "—"}</dd></div>
     <div><dt>Source grid</dt><dd>${grid}</dd></div>
     <div><dt>Electrical matrix</dt><dd>${matrix}</dd></div>
     <div><dt>HID usage</dt><dd>${hid}</dd></div>
     <div><dt>Device key index</dt><dd>${key.deviceKeyIndex ?? "—"}</dd></div>
     <div><dt>LED index</dt><dd>${key.ledIndex ?? "—"}</dd></div>
     <div><dt>Fn programmable</dt><dd>${key.fnProgrammable ? "Yes" : "No"}</dd></div>
     <div><dt>Default action</dt><dd>${actionLabel(key.defaultAction)}</dd></div>`;
}

async function loadPrimaryLayout() {
  try {
    const layout = await invoke("get_primary_layout");
    renderKeyboard(layout);
  } catch (error) {
    $("#keyboardViewport").innerHTML =
      `<div class="layout-loading">Could not load layout: ${String(error)}</div>`;
    $("#layoutName").textContent = "Layout unavailable";
  }
}

$("#applyLighting").addEventListener("click", async () => {
  const [r, g, b] = hexRgb($("#lightingColor").value);
  const config = {
    mode: Number($("#lightingMode").value),
    r,
    g,
    b,
    brightness: Number($("#brightness").value),
    speed: Number($("#speed").value),
    direction: Number($("#direction").value),
    colorful: $("#colorful").checked
  };

  $("#lightingResult").textContent = "Applying...";
  $("#applyLighting").disabled = true;

  try {
    const result = await invoke("apply_lighting", { config });
    if (result.applied) {
      $("#lightingResult").textContent = "Desired state saved and applied.";
    } else {
      $("#lightingResult").textContent =
        "Desired state saved. Keyboard not applied: " + (result.message || "not ready");
    }
    await loadPersistence();
    await loadDesiredLighting();
  } catch (error) {
    $("#lightingResult").textContent = "Could not save desired state: " + error;
  }
  await pollStatus();
});

$("#reconnectButton").addEventListener("click", async () => {
  await invoke("reconnect");
  setTimeout(pollStatus, 250);
});

async function refreshLogs() {
  try {
    const [logs, drivers, persistence] = await Promise.all([
      invoke("get_logs"),
      invoke("get_driver_registry"),
      invoke("get_persistence_status")
    ]);
    $("#persistenceView").textContent =
      `SQLite | schema ${persistence.schemaVersion} | ${persistence.journalMode} | integrity ${persistence.integrityOk ? "OK" : "FAILED"}\n${persistence.path}`;
    $("#driverView").textContent = drivers.length
      ? drivers.map((driver) => {
          const matches = driver.matchers
            .map((match) => {
              const vid = match.vendorId.toString(16).padStart(4, "0").toUpperCase();
              const pid = match.productId.toString(16).padStart(4, "0").toUpperCase();
              const usage = match.usagePage == null
                ? "any usage"
                : "usage " + match.usagePage.toString(16).padStart(4, "0").toUpperCase();
              return `${vid}:${pid} ${usage}`;
            })
            .join(", ");
          const layouts = (driver.layoutIds || []).join(", ") || "no layout";
          return `${driver.driverId} | ${driver.deviceName} | layouts ${layouts} | ${matches}`;
        }).join("\n")
      : "No drivers registered.";

    $("#logView").textContent = logs.length
      ? logs.map((entry) => {
          const time = new Date(Number(entry.unixMillis)).toLocaleTimeString();
          return `${time} [${entry.level.toUpperCase()}] ${entry.message}`;
        }).join("\n")
      : "No log entries yet.";
  } catch (error) {
    $("#logView").textContent = String(error);
  }
}

$("#refreshLogs").addEventListener("click", refreshLogs);

(async function boot() {
  applyTheme(initialTheme());
  await Promise.all([
    loadLightingEffects(),
    loadPersistence(),
    loadDesiredLighting(),
    loadPrimaryLayout()
  ]);
  await pollStatus();

  setTimeout(() => {
    $("#splash").classList.add("hidden");
    $("#app").classList.remove("hidden");
  }, 850);

  setInterval(pollStatus, 1200);
})();
