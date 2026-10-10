const keymapInvoke = (command, args = {}) => window.__TAURI__.core.invoke(command, args);
const keymapQ = (selector) => document.querySelector(selector);

let keymapLayout = null;
let keymapCapabilities = null;
let keymapMappings = new Map();
let keymapClipboard = null;
let keymapHistory = [];
let keymapHistoryIndex = -1;
let savedKeymapSnapshot = "[]";
let keymapInitialized = false;

function deepClone(value) {
  return value == null ? value : JSON.parse(JSON.stringify(value));
}

function mappingSnapshot() {
  return JSON.stringify(
    [...keymapMappings.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([keyId, action]) => ({ keyId, action }))
  );
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
  return keymapMappings.get(keyId) || { type: "default" };
}

function actionsEqual(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
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
      element.textContent = `${option.label} (0x${option.usage.toString(16).padStart(2, "0").toUpperCase()})`;
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

  populateSelect(
    "#mediaTarget",
    keymapCapabilities.mediaActions,
    (value) => mediaLabels[value] || titleCase(value)
  );
  populateSelect(
    "#mouseTarget",
    keymapCapabilities.mouseActions,
    (value) => mouseLabels[value] || titleCase(value)
  );
  populateSelect(
    "#windowsTarget",
    keymapCapabilities.windowsActions,
    (value) => windowsLabels[value] || titleCase(value)
  );

  const typeSelect = keymapQ("#keyActionType");
  setOptionEnabled(typeSelect, "disabled", keymapCapabilities.disable);
  setOptionEnabled(typeSelect, "keyboard-key", keymapCapabilities.keyboardKey);
  setOptionEnabled(typeSelect, "modifier", keymapCapabilities.modifier);
  setOptionEnabled(typeSelect, "shortcut", keymapCapabilities.shortcut);
  setOptionEnabled(typeSelect, "media", keymapCapabilities.mediaActions.length > 0);
  setOptionEnabled(typeSelect, "mouse", keymapCapabilities.mouseActions.length > 0);
  setOptionEnabled(typeSelect, "windows", keymapCapabilities.windowsActions.length > 0);
  setOptionEnabled(typeSelect, "open-application", keymapCapabilities.openApplication);
  setOptionEnabled(typeSelect, "open-file", keymapCapabilities.openFile);
  setOptionEnabled(typeSelect, "open-url", keymapCapabilities.openUrl);
  setOptionEnabled(typeSelect, "send-text", keymapCapabilities.sendText);
  setOptionEnabled(typeSelect, "macro", keymapCapabilities.macroAction);

  if (keymapCapabilities.unsupportedHostActionsReason) {
    keymapQ("#hostActionNotice").textContent = keymapCapabilities.unsupportedHostActionsReason;
  }
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
      throw new Error("This action is not currently supported by the F108 R6 driver.");
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
        const checkbox = document.querySelector(`#shortcutFields input[value="${CSS.escape(modifier)}"]`);
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
  keymapQ("#selectedKeyCount").textContent = count === 0
    ? "No keys selected"
    : `${count} key${count === 1 ? "" : "s"} selected`;
  keymapQ("#applyKeyAction").disabled = count === 0;
  keymapQ("#copyKeyAction").disabled = count !== 1;
  keymapQ("#pasteKeyAction").disabled = count === 0 || !keymapClipboard;
  keymapQ("#resetSelectedKeys").disabled = count === 0;

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
  document.querySelectorAll(".keyboard-key").forEach((element) => {
    const action = actionForKey(element.dataset.keyId);
    const mapped = action.type !== "default";
    element.classList.toggle("mapped", mapped);
    element.dataset.assignment = actionSummary(action);
  });
}

function commitHistory() {
  const snapshot = mappingSnapshot();
  if (keymapHistory[keymapHistoryIndex] === snapshot) return;
  keymapHistory = keymapHistory.slice(0, keymapHistoryIndex + 1);
  keymapHistory.push(snapshot);
  keymapHistoryIndex = keymapHistory.length - 1;
  updateHistoryButtons();
  updateDirtyState();
}

function restoreSnapshot(snapshot) {
  keymapMappings = new Map(
    JSON.parse(snapshot).map((mapping) => [mapping.keyId, mapping.action])
  );
  refreshMappedKeys();
  refreshSelectionEditor();
  updateDirtyState();
}

function updateHistoryButtons() {
  keymapQ("#undoKeymap").disabled = keymapHistoryIndex <= 0;
  keymapQ("#redoKeymap").disabled = keymapHistoryIndex >= keymapHistory.length - 1;
}

function updateDirtyState(message) {
  const dirty = mappingSnapshot() !== savedKeymapSnapshot;
  if (message) {
    keymapQ("#keymapStatus").textContent = message;
  } else {
    keymapQ("#keymapStatus").textContent = dirty
      ? "Unsaved normal-layer changes"
      : `${keymapMappings.size} mapped key${keymapMappings.size === 1 ? "" : "s"} saved`;
  }
  keymapQ("#saveNormalKeymap").classList.toggle("dirty", dirty);
}

function applyActionToSelection(action) {
  const ids = selectedKeyIdsFromDom();
  if (ids.length === 0) return;
  for (const keyId of ids) {
    if (action.type === "default") keymapMappings.delete(keyId);
    else keymapMappings.set(keyId, deepClone(action));
  }
  commitHistory();
  refreshMappedKeys();
  refreshSelectionEditor();
}

async function saveNormalKeymap() {
  const mappings = [...keymapMappings.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([keyId, action]) => ({ keyId, action }));
  const button = keymapQ("#saveNormalKeymap");
  button.disabled = true;
  updateDirtyState("Validating and writing the complete normal layer...");
  try {
    const result = await keymapInvoke("save_normal_keymap", { mappings });
    savedKeymapSnapshot = mappingSnapshot();
    updateDirtyState(
      result.applied
        ? `Saved and applied ${result.assignmentCount} mapping${result.assignmentCount === 1 ? "" : "s"}. Physical key verification is still required.`
        : `Desired keymap saved locally. Keyboard not applied: ${result.message || "not ready"}`
    );
  } catch (error) {
    updateDirtyState(`Keymap rejected: ${String(error)}`);
  } finally {
    button.disabled = false;
  }
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
    applyActionToSelection({ type: "default" });
  });
  keymapQ("#copyKeyAction").addEventListener("click", () => {
    const keys = selectedKeys();
    if (keys.length !== 1) return;
    keymapClipboard = deepClone(actionForKey(keys[0].id));
    keymapQ("#pasteKeyAction").disabled = false;
    updateDirtyState(`Copied ${actionSummary(keymapClipboard)}.`);
  });
  keymapQ("#pasteKeyAction").addEventListener("click", () => {
    if (keymapClipboard) applyActionToSelection(keymapClipboard);
  });
  keymapQ("#undoKeymap").addEventListener("click", () => {
    if (keymapHistoryIndex <= 0) return;
    keymapHistoryIndex -= 1;
    restoreSnapshot(keymapHistory[keymapHistoryIndex]);
    updateHistoryButtons();
  });
  keymapQ("#redoKeymap").addEventListener("click", () => {
    if (keymapHistoryIndex >= keymapHistory.length - 1) return;
    keymapHistoryIndex += 1;
    restoreSnapshot(keymapHistory[keymapHistoryIndex]);
    updateHistoryButtons();
  });
  keymapQ("#saveNormalKeymap").addEventListener("click", saveNormalKeymap);

  const viewport = keymapQ("#keyboardViewport");
  viewport.addEventListener("click", () => setTimeout(refreshSelectionEditor, 0));
  viewport.addEventListener("keydown", () => setTimeout(refreshSelectionEditor, 0));
  new MutationObserver(() => refreshMappedKeys()).observe(viewport, {
    childList: true,
    subtree: true
  });
}

async function initializeKeymapEditor() {
  if (keymapInitialized) return;
  keymapInitialized = true;
  bindKeymapEvents();

  try {
    const [layout, capabilities, desired] = await Promise.all([
      keymapInvoke("get_primary_layout"),
      keymapInvoke("get_normal_remap_capabilities"),
      keymapInvoke("get_desired_normal_keymap")
    ]);
    keymapLayout = layout;
    keymapCapabilities = capabilities;
    keymapMappings = new Map(
      (desired.mappings || []).map((mapping) => [mapping.keyId, mapping.action])
    );

    populateKeyTargets();
    populateCapabilityTargets();
    updateActionFields();
    const initial = mappingSnapshot();
    keymapHistory = [initial];
    keymapHistoryIndex = 0;
    savedKeymapSnapshot = initial;
    updateHistoryButtons();
    refreshMappedKeys();
    refreshSelectionEditor();
    updateDirtyState();
  } catch (error) {
    keymapQ("#keymapStatus").textContent = `Could not initialize normal remapping: ${String(error)}`;
    keymapQ("#saveNormalKeymap").disabled = true;
  }
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initializeKeymapEditor, { once: true });
} else {
  initializeKeymapEditor();
}
