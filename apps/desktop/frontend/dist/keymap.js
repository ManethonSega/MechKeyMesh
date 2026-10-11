const keymapInvoke = (command, args = {}) => window.__TAURI__.core.invoke(command, args);
const keymapQ = (selector) => document.querySelector(selector);

let keymapLayout = null;
let fnLayerDescriptor = null;
let activeKeymapLayer = "normal";
let keymapClipboard = null;
let keymapInitialized = false;

const layerStates = {
  normal: null,
  fn: null
};

function deepClone(value) {
  return value == null ? value : JSON.parse(JSON.stringify(value));
}

function layerLabel(layer = activeKeymapLayer) {
  return layer === "fn" ? "Fn" : "Normal";
}

function createLayerState(desired, capabilities) {
  const mappings = new Map(
    (desired.mappings || []).map((mapping) => [mapping.keyId, mapping.action])
  );
  const initial = snapshotForMap(mappings);
  return {
    mappings,
    capabilities,
    history: [initial],
    historyIndex: 0,
    savedSnapshot: initial
  };
}

function activeState() {
  return layerStates[activeKeymapLayer];
}

function snapshotForMap(mappings) {
  return JSON.stringify(
    [...mappings.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([keyId, action]) => ({ keyId, action }))
  );
}

function mappingSnapshot() {
  const state = activeState();
  return state ? snapshotForMap(state.mappings) : "[]";
}

function selectedKeyIdsFromDom() {
  return [...document.querySelectorAll(".keyboard-key.selected")]
    .map((element) => element.dataset.keyId)
    .filter(Boolean);
}

function selectedKeys() {
  if (!keymapLayout) return [];
  const ids = new Set(selectedKeyIdsFromDom());
  return keymapLayout.keys.filter((key) => ids.has(key.id));
}

function actionForKey(keyId) {
  return activeState()?.mappings.get(keyId) || { type: "default" };
}

function actionsEqual(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function restrictionForKey(key) {
  if (activeKeymapLayer !== "fn" || !key) return null;
  const index = Number(key.deviceKeyIndex);
  return activeState()?.capabilities?.protectedKeys?.find(
    (restriction) => Number(restriction.deviceKeyIndex) === index
  ) || null;
}

function actionSummary(action) {
  if (!action) return "Default";
  const labels = {
    "default": "Default",
    "disabled": "Disabled",
    "keyboard-key": "Keyboard key",
    "modified-key": "Shortcut",
    "media": "Media",
    "mouse": "Mouse",
    "windows": "Windows / system",
    "open-application": "Open application",
    "open-file": "Open file",
    "open-url": "Open website",
    "send-text": "Send text",
    "macro": "Macro"
  };
  switch (action.type) {
    case "keyboard-key":
      return `Keyboard HID 0x${Number(action.usage).toString(16).padStart(2, "0").toUpperCase()}`;
    case "modified-key":
      return `${(action.modifiers || []).join(" + ")} + HID 0x${Number(action.key).toString(16).padStart(2, "0").toUpperCase()}`;
    case "media":
    case "mouse":
    case "windows":
      return `${labels[action.type]}: ${action.action}`;
    default:
      return labels[action.type] || action.type || "Default";
  }
}

function populateKeyTargets() {
  if (!keymapLayout) return;
  const seen = new Set();
  const options = [];
  for (const key of keymapLayout.keys) {
    const usage = Number(key.hidUsage);
    if (!Number.isInteger(usage) || usage < 1 || usage > 255 || usage === 0xAF) continue;
    if (seen.has(usage)) continue;
    seen.add(usage);
    options.push({ usage, label: key.label });
  }

  for (const selector of ["#keyboardKeyTarget", "#shortcutKeyTarget"]) {
    const select = keymapQ(selector);
    select.innerHTML = "";
    for (const option of options) {
      const element = document.createElement("option");
      element.value = String(option.usage);
      element.textContent =
        `${option.label} (0x${option.usage.toString(16).padStart(2, "0").toUpperCase()})`;
      select.appendChild(element);
    }
  }
}

function titleCase(value) {
  return String(value)
    .split("-")
    .map((part) => part ? part[0].toUpperCase() + part.slice(1) : part)
    .join(" ");
}

function populateCapabilityTargets() {
  const capabilities = activeState()?.capabilities;
  if (!capabilities) return;

  const mediaLabels = {
    "play-pause": "Play / Pause",
    "stop": "Stop",
    "previous": "Previous track",
    "next": "Next track",
    "volume-up": "Volume up",
    "volume-down": "Volume down",
    "mute": "Mute"
  };
  const mouseLabels = {
    "left-click": "Left click",
    "middle-click": "Middle click",
    "right-click": "Right click",
    "scroll-up": "Scroll up",
    "scroll-down": "Scroll down"
  };
  const windowsLabels = {
    "show-desktop": "Show desktop (Win+D)",
    "computer": "Open File Explorer (Win+E)",
    "lock": "Lock PC (Win+L)",
    "close-window": "Close window (Ctrl+W)",
    "switch-window": "Switch window (Alt+Tab)",
    "calculator": "Calculator"
  };

  populateSelect("#mediaTarget", capabilities.mediaActions, (value) => mediaLabels[value] || titleCase(value));
  populateSelect("#mouseTarget", capabilities.mouseActions, (value) => mouseLabels[value] || titleCase(value));
  populateSelect("#windowsTarget", capabilities.windowsActions, (value) => windowsLabels[value] || titleCase(value));

  const typeSelect = keymapQ("#keyActionType");
  setOptionEnabled(typeSelect, "disabled", capabilities.disable);
  setOptionEnabled(typeSelect, "keyboard-key", capabilities.keyboardKey);
  setOptionEnabled(typeSelect, "modifier", capabilities.modifier);
  setOptionEnabled(typeSelect, "shortcut", capabilities.shortcut);
  setOptionEnabled(typeSelect, "media", capabilities.mediaActions.length > 0);
  setOptionEnabled(typeSelect, "mouse", capabilities.mouseActions.length > 0);
  setOptionEnabled(typeSelect, "windows", capabilities.windowsActions.length > 0);
  setOptionEnabled(typeSelect, "open-application", capabilities.openApplication);
  setOptionEnabled(typeSelect, "open-file", capabilities.openFile);
  setOptionEnabled(typeSelect, "open-url", capabilities.openUrl);
  setOptionEnabled(typeSelect, "send-text", capabilities.sendText);
  setOptionEnabled(typeSelect, "macro", capabilities.macroAction);

  if (typeSelect.selectedOptions[0]?.disabled) typeSelect.value = "default";
  if (capabilities.unsupportedHostActionsReason) {
    keymapQ("#hostActionNotice").textContent = capabilities.unsupportedHostActionsReason;
  }
  updateActionFields();
}

function populateSelect(selector, values, labeler) {
  const select = keymapQ(selector);
  select.innerHTML = "";
  for (const value of values || []) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = labeler(value);
    select.appendChild(option);
  }
}

function setOptionEnabled(select, value, enabled) {
  const option = [...select.options].find((item) => item.value === value);
  if (option) option.disabled = !enabled;
}

function updateActionFields() {
  const type = keymapQ("#keyActionType").value;
  const mapping = {
    "keyboard-key": "#keyboardKeyFields",
    "modifier": "#modifierFields",
    "shortcut": "#shortcutFields",
    "media": "#mediaFields",
    "mouse": "#mouseFields",
    "windows": "#windowsFields"
  };
  for (const selector of Object.values(mapping)) {
    keymapQ(selector).classList.add("hidden-control");
  }
  if (mapping[type]) keymapQ(mapping[type]).classList.remove("hidden-control");
}

function actionFromEditor() {
  const type = keymapQ("#keyActionType").value;
  switch (type) {
    case "default":
      return { type: "default" };
    case "disabled":
      return { type: "disabled" };
    case "keyboard-key":
      return { type: "keyboard-key", usage: Number(keymapQ("#keyboardKeyTarget").value) };
    case "modifier":
      return { type: "keyboard-key", usage: Number(keymapQ("#modifierTarget").value) };
    case "shortcut": {
      const modifiers = [...document.querySelectorAll("#shortcutFields input[type=checkbox]:checked")]
        .map((input) => input.value);
      if (modifiers.length === 0) throw new Error("Select at least one shortcut modifier.");
      return {
        type: "modified-key",
        modifiers,
        key: Number(keymapQ("#shortcutKeyTarget").value)
      };
    }
    case "media":
      return { type: "media", action: keymapQ("#mediaTarget").value };
    case "mouse":
      return { type: "mouse", action: keymapQ("#mouseTarget").value };
    case "windows":
      return { type: "windows", action: keymapQ("#windowsTarget").value };
    default:
      throw new Error("This action is not currently supported by the F108 driver.");
  }
}

function editorFromAction(action) {
  document.querySelectorAll("#shortcutFields input[type=checkbox]").forEach((input) => {
    input.checked = false;
  });

  switch (action?.type) {
    case "disabled":
      keymapQ("#keyActionType").value = "disabled";
      break;
    case "keyboard-key":
      if (Number(action.usage) >= 0xE0 && Number(action.usage) <= 0xE7) {
        keymapQ("#keyActionType").value = "modifier";
        keymapQ("#modifierTarget").value = String(action.usage);
      } else {
        keymapQ("#keyActionType").value = "keyboard-key";
        keymapQ("#keyboardKeyTarget").value = String(action.usage);
      }
      break;
    case "modified-key":
      keymapQ("#keyActionType").value = "shortcut";
      keymapQ("#shortcutKeyTarget").value = String(action.key);
      for (const modifier of action.modifiers || []) {
        const checkbox = document.querySelector(
          `#shortcutFields input[value="${CSS.escape(modifier)}"]`
        );
        if (checkbox) checkbox.checked = true;
      }
      break;
    case "media":
      keymapQ("#keyActionType").value = "media";
      keymapQ("#mediaTarget").value = action.action;
      break;
    case "mouse":
      keymapQ("#keyActionType").value = "mouse";
      keymapQ("#mouseTarget").value = action.action;
      break;
    case "windows":
      keymapQ("#keyActionType").value = "windows";
      keymapQ("#windowsTarget").value = action.action;
      break;
    default:
      keymapQ("#keyActionType").value = "default";
  }
  updateActionFields();
}

function refreshSelectionEditor() {
  const keys = selectedKeys();
  const count = keys.length;
  const restrictions = keys
    .map((key) => ({ key, restriction: restrictionForKey(key) }))
    .filter((entry) => entry.restriction);

  keymapQ("#selectedKeyCount").textContent = count === 0
    ? "No keys selected"
    : `${count} key${count === 1 ? "" : "s"} selected`;

  const hasProtected = restrictions.length > 0;
  keymapQ("#applyKeyAction").disabled = count === 0 || hasProtected;
  keymapQ("#copyKeyAction").disabled = count !== 1;
  keymapQ("#pasteKeyAction").disabled = count === 0 || !keymapClipboard || hasProtected;
  keymapQ("#resetSelectedKeys").disabled = count === 0;

  const notice = keymapQ("#fnProtectionNotice");
  if (hasProtected) {
    const details = restrictions
      .map(({ key, restriction }) => `${key.label}: ${restriction.reason}`)
      .join(" ");
    notice.textContent =
      "Protected Fn shortcut. MechKeyMesh blocks non-default assignments here to preserve firmware behavior. " +
      details;
    notice.classList.remove("hidden-control");
  } else {
    notice.classList.add("hidden-control");
    notice.textContent = "";
  }

  if (count === 0) {
    keymapQ("#currentAssignment").textContent = "Assigned action: Default";
    return;
  }

  const actions = keys.map((key) => actionForKey(key.id));
  const first = actions[0];
  const same = actions.every((action) => actionsEqual(action, first));
  keymapQ("#currentAssignment").textContent = same
    ? `Assigned action: ${actionSummary(first)}`
    : "Assigned action: Mixed selection";
  if (same) editorFromAction(first);
}

function refreshMappedKeys() {
  const state = activeState();
  if (!state) return;

  document.querySelectorAll(".keyboard-key").forEach((element) => {
    const keyId = element.dataset.keyId;
    const key = keymapLayout?.keys.find((item) => item.id === keyId);
    const action = actionForKey(keyId);
    const mapped = action.type !== "default";
    const restriction = restrictionForKey(key);

    element.classList.toggle("mapped", mapped);
    element.classList.toggle("fn-protected", Boolean(restriction));
    element.dataset.assignment = actionSummary(action);
    if (restriction) {
      element.dataset.protectionReason = restriction.reason;
      element.setAttribute("aria-disabled", "true");
    } else {
      delete element.dataset.protectionReason;
      element.removeAttribute("aria-disabled");
    }
  });
}

function commitHistory() {
  const state = activeState();
  if (!state) return;

  const snapshot = mappingSnapshot();
  if (state.history[state.historyIndex] === snapshot) return;
  state.history = state.history.slice(0, state.historyIndex + 1);
  state.history.push(snapshot);
  state.historyIndex = state.history.length - 1;
  updateHistoryButtons();
  updateDirtyState();
}

function restoreSnapshot(snapshot) {
  const state = activeState();
  if (!state) return;

  state.mappings = new Map(
    JSON.parse(snapshot).map((mapping) => [mapping.keyId, mapping.action])
  );
  refreshMappedKeys();
  refreshSelectionEditor();
  updateDirtyState();
}

function updateHistoryButtons() {
  const state = activeState();
  keymapQ("#undoKeymap").disabled = !state || state.historyIndex <= 0;
  keymapQ("#redoKeymap").disabled =
    !state || state.historyIndex >= state.history.length - 1;
}

function updateDirtyState(message) {
  const state = activeState();
  if (!state) return;

  const dirty = mappingSnapshot() !== state.savedSnapshot;
  if (message) {
    keymapQ("#keymapStatus").textContent = message;
  } else {
    keymapQ("#keymapStatus").textContent = dirty
      ? `Unsaved ${layerLabel()}-layer changes`
      : `${state.mappings.size} mapped key${state.mappings.size === 1 ? "" : "s"} saved on ${layerLabel()}`;
  }
  keymapQ("#saveKeymap").classList.toggle("dirty", dirty);
}

function applyActionToSelection(action) {
  const state = activeState();
  const keys = selectedKeys();
  if (!state || keys.length === 0) return;

  const protectedKey = keys.find((key) => restrictionForKey(key));
  if (protectedKey && action.type !== "default") {
    throw new Error(
      `${protectedKey.label} is protected on the Fn layer: ${restrictionForKey(protectedKey).reason}`
    );
  }

  for (const key of keys) {
    if (action.type === "default") state.mappings.delete(key.id);
    else state.mappings.set(key.id, deepClone(action));
  }
  commitHistory();
  refreshMappedKeys();
  refreshSelectionEditor();
}

async function saveCurrentKeymap(statusText = null) {
  const state = activeState();
  if (!state) return false;

  const mappings = [...state.mappings.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([keyId, action]) => ({ keyId, action }));
  const button = keymapQ("#saveKeymap");
  button.disabled = true;
  updateDirtyState(
    statusText || `Validating and writing the complete ${layerLabel()} layer...`
  );

  try {
    const result = await keymapInvoke("save_keymap", {
      layer: activeKeymapLayer,
      mappings
    });
    state.savedSnapshot = mappingSnapshot();
    updateDirtyState(
      result.applied
        ? `Saved and applied ${result.assignmentCount} ${layerLabel()} mapping${result.assignmentCount === 1 ? "" : "s"}. Physical key verification is still required.`
        : `Desired ${layerLabel()} keymap saved locally. Keyboard not applied: ${result.message || "not ready"}`
    );
    return true;
  } catch (error) {
    updateDirtyState(`Keymap rejected: ${String(error)}`);
    return false;
  } finally {
    button.disabled = false;
  }
}

async function resetCurrentLayer() {
  const state = activeState();
  if (!state) return;
  if (!window.confirm(
    `Reset the entire ${layerLabel()} layer to firmware default? This clears all saved assignments on this layer.`
  )) return;

  state.mappings.clear();
  commitHistory();
  refreshMappedKeys();
  refreshSelectionEditor();
  await saveCurrentKeymap(`Resetting the complete ${layerLabel()} remap table...`);
}

async function resetAllKeyAssignments() {
  if (!window.confirm(
    "Reset both Normal and Fn key assignments? Lighting, LCD media, device settings and other subsystems are not reset."
  )) return;

  const button = keymapQ("#resetAllKeyAssignments");
  button.disabled = true;
  updateDirtyState("Resetting both key assignment layers...");
  try {
    const result = await keymapInvoke("reset_key_assignments");
    for (const layer of ["normal", "fn"]) {
      const state = layerStates[layer];
      state.mappings.clear();
      state.history = ["[]"];
      state.historyIndex = 0;
      state.savedSnapshot = "[]";
    }
    refreshMappedKeys();
    refreshSelectionEditor();
    updateHistoryButtons();
    updateDirtyState(
      result.normalApplied && result.fnApplied
        ? "Normal and Fn key assignments reset and applied."
        : `Desired Normal and Fn assignments reset locally. ${result.message || "Hardware application is pending."}`
    );
  } catch (error) {
    updateDirtyState(`Could not reset key assignments: ${String(error)}`);
  } finally {
    button.disabled = false;
  }
}

function updateFnLayerInfo() {
  const panel = keymapQ("#fnLayerInfo");
  if (activeKeymapLayer !== "fn") {
    panel.classList.add("hidden-control");
    return;
  }

  panel.classList.remove("hidden-control");
  if (!fnLayerDescriptor) {
    keymapQ("#fnModeSummary").textContent = "Fn activation metadata unavailable.";
    return;
  }

  const modes = (fnLayerDescriptor.modes || [])
    .map((option) => `${titleCase(option.mode)} = raw ${option.rawValue}`)
    .join(", ");
  const current = fnLayerDescriptor.currentMode
    ? titleCase(fnLayerDescriptor.currentMode)
    : "unavailable";
  keymapQ("#fnModeSummary").textContent =
    `OEM enum: ${modes}. Current device value: ${current}. ${fnLayerDescriptor.note}`;
}

function switchLayer(layer) {
  if (!layerStates[layer]) return;
  activeKeymapLayer = layer;

  document.querySelectorAll(".layer-tab[data-layer]").forEach((button) => {
    const active = button.dataset.layer === layer;
    button.classList.toggle("active", active);
    button.setAttribute("aria-selected", String(active));
  });

  keymapQ("#actionEditorTitle").textContent = `${layerLabel()}-layer action`;
  keymapQ("#saveKeymap").textContent = `Save desired ${layerLabel()} layer`;

  populateCapabilityTargets();
  updateFnLayerInfo();
  updateHistoryButtons();
  refreshMappedKeys();
  refreshSelectionEditor();
  updateDirtyState();
}

function bindKeymapEvents() {
  keymapQ("#keyActionType").addEventListener("change", updateActionFields);
  keymapQ("#applyKeyAction").addEventListener("click", () => {
    try {
      applyActionToSelection(actionFromEditor());
    } catch (error) {
      updateDirtyState(String(error));
    }
  });
  keymapQ("#resetSelectedKeys").addEventListener("click", () => {
    try {
      applyActionToSelection({ type: "default" });
    } catch (error) {
      updateDirtyState(String(error));
    }
  });
  keymapQ("#copyKeyAction").addEventListener("click", () => {
    const keys = selectedKeys();
    if (keys.length !== 1) return;
    keymapClipboard = deepClone(actionForKey(keys[0].id));
    keymapQ("#pasteKeyAction").disabled = false;
    updateDirtyState(`Copied ${actionSummary(keymapClipboard)}.`);
  });
  keymapQ("#pasteKeyAction").addEventListener("click", () => {
    try {
      if (keymapClipboard) applyActionToSelection(keymapClipboard);
    } catch (error) {
      updateDirtyState(String(error));
    }
  });
  keymapQ("#undoKeymap").addEventListener("click", () => {
    const state = activeState();
    if (!state || state.historyIndex <= 0) return;
    state.historyIndex -= 1;
    restoreSnapshot(state.history[state.historyIndex]);
    updateHistoryButtons();
  });
  keymapQ("#redoKeymap").addEventListener("click", () => {
    const state = activeState();
    if (!state || state.historyIndex >= state.history.length - 1) return;
    state.historyIndex += 1;
    restoreSnapshot(state.history[state.historyIndex]);
    updateHistoryButtons();
  });
  keymapQ("#saveKeymap").addEventListener("click", () => void saveCurrentKeymap());
  keymapQ("#resetCurrentLayer").addEventListener("click", () => void resetCurrentLayer());
  keymapQ("#resetAllKeyAssignments").addEventListener("click", () => void resetAllKeyAssignments());

  document.querySelectorAll(".layer-tab[data-layer]").forEach((button) => {
    button.addEventListener("click", () => switchLayer(button.dataset.layer));
  });

  const viewport = keymapQ("#keyboardViewport");
  viewport.addEventListener("click", () => setTimeout(refreshSelectionEditor, 0));
  viewport.addEventListener("keydown", () => setTimeout(refreshSelectionEditor, 0));
  new MutationObserver(() => {
    refreshMappedKeys();
    refreshSelectionEditor();
  }).observe(viewport, {
    childList: true,
    subtree: true
  });

  window.addEventListener("mechkeymesh-layout-changed", (event) => {
    keymapLayout = event.detail?.layout || keymapLayout;
    populateKeyTargets();
    refreshMappedKeys();
    refreshSelectionEditor();
  });
}

async function loadLayerState(layer) {
  const [capabilities, desired] = await Promise.all([
    keymapInvoke("get_keymap_capabilities", { layer }),
    keymapInvoke("get_desired_keymap", { layer })
  ]);
  return createLayerState(desired, capabilities);
}

async function initializeKeymapEditor() {
  if (keymapInitialized) return;
  keymapInitialized = true;
  bindKeymapEvents();

  try {
    const [layout, normal, fnLayer, descriptor] = await Promise.all([
      keymapInvoke("get_primary_layout"),
      loadLayerState("normal"),
      loadLayerState("fn"),
      keymapInvoke("get_fn_layer_descriptor")
    ]);

    keymapLayout = layout;
    layerStates.normal = normal;
    layerStates.fn = fnLayer;
    fnLayerDescriptor = descriptor;

    populateKeyTargets();
    switchLayer("normal");
  } catch (error) {
    keymapQ("#keymapStatus").textContent =
      `Could not initialize Normal/Fn remapping: ${String(error)}`;
    keymapQ("#saveKeymap").disabled = true;
  }
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initializeKeymapEditor, { once: true });
} else {
  initializeKeymapEditor();
}
