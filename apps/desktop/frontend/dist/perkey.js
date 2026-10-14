const perKeyInvoke = (command, args = {}) => window.__TAURI__.core.invoke(command, args);
const perKeyQ = (selector) => document.querySelector(selector);
const PER_KEY_SVG_NS = "http://www.w3.org/2000/svg";

let perKeyLayout = null;
let perKeyCapabilities = null;
let perKeyColors = new Map();
let perKeySelected = new Set();
let perKeyClipboard = null;
let perKeyRegionMode = false;
let perKeyRegionStart = null;
let perKeyRegionRect = null;
let perKeySuppressNextClick = false;
let perKeySavedSnapshot = "";
let perKeyInitialized = false;

function perKeyColorHex(color) {
  const component = (value) => Number(value).toString(16).padStart(2, "0");
  return `#${component(color?.r ?? 0)}${component(color?.g ?? 0)}${component(color?.b ?? 0)}`;
}

function perKeyHexColor(hex) {
  const value = Number.parseInt(String(hex).replace("#", ""), 16);
  return {
    r: (value >> 16) & 255,
    g: (value >> 8) & 255,
    b: value & 255
  };
}

function perKeyIsBlack(color) {
  return !color || (color.r === 0 && color.g === 0 && color.b === 0);
}

function perKeySnapshot() {
  return JSON.stringify({
    brightness: Number(perKeyQ("#perKeyBrightness")?.value ?? 5),
    colors: [...perKeyColors.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([keyId, color]) => ({ keyId, color }))
  });
}

function perKeyUpdateDirty(message = null) {
  const dirty = perKeySnapshot() !== perKeySavedSnapshot;
  const status = perKeyQ("#perKeyStatus");
  if (message) {
    status.textContent = message;
  } else {
    status.textContent = dirty
      ? "Unsaved per-key RGB changes"
      : `${perKeyColors.size} lit key${perKeyColors.size === 1 ? "" : "s"} saved`;
  }
  perKeyQ("#savePerKeyRgb").classList.toggle("dirty", dirty);
}

function perKeyOrderedSelection() {
  if (!perKeyLayout) return [];
  return perKeyLayout.keys
    .filter((key) => perKeySelected.has(key.id) && key.ledIndex != null)
    .sort((left, right) => {
      const leftCenter = perKeyRotatedCenter(left);
      const rightCenter = perKeyRotatedCenter(right);
      return leftCenter.x - rightCenter.x ||
        leftCenter.y - rightCenter.y ||
        left.id.localeCompare(right.id);
    });
}

function perKeySetSelection(keys, additive = false) {
  if (!additive) perKeySelected.clear();
  for (const key of keys) {
    if (key.ledIndex != null) perKeySelected.add(key.id);
  }
  perKeyRefreshSelection();
}

function perKeyToggleSelection(key, additive) {
  if (key.ledIndex == null) return;
  if (!additive) {
    perKeySelected.clear();
    perKeySelected.add(key.id);
  } else if (perKeySelected.has(key.id)) {
    perKeySelected.delete(key.id);
  } else {
    perKeySelected.add(key.id);
  }
  perKeyRefreshSelection();
}

function perKeyRefreshSelection() {
  document.querySelectorAll("#perKeyKeyboardViewport .perkey-key").forEach((element) => {
    const selected = perKeySelected.has(element.dataset.keyId);
    element.classList.toggle("selected", selected);
    element.setAttribute("aria-pressed", String(selected));
  });

  const count = perKeySelected.size;
  perKeyQ("#perKeySelectionCount").textContent =
    count === 0 ? "No keys selected" : `${count} key${count === 1 ? "" : "s"} selected`;
  perKeyQ("#applyPerKeyColor").disabled = count === 0;
  perKeyQ("#applyPerKeyGradient").disabled = count < 2;
  perKeyQ("#copyPerKeyColor").disabled = count === 0;
  perKeyQ("#pastePerKeyColor").disabled = count === 0 || !perKeyClipboard;
  perKeyQ("#clearPerKeyColor").disabled = count === 0;
}

function perKeyRefreshPreview() {
  document.querySelectorAll("#perKeyKeyboardViewport .perkey-key").forEach((element) => {
    const color = perKeyColors.get(element.dataset.keyId) || { r: 0, g: 0, b: 0 };
    const swatch = element.querySelector(".perkey-led-swatch");
    if (swatch) swatch.setAttribute("fill", perKeyColorHex(color));
    element.classList.toggle("perkey-lit", !perKeyIsBlack(color));
    element.dataset.rgb = perKeyColorHex(color).toUpperCase();
  });
}

function perKeyFontSize(key) {
  const labelLength = Math.max(String(key.label || "").length, 1);
  return Math.max(5, Math.min(8, key.width / (labelLength * 0.72), key.height * 0.32));
}

function perKeyRender(layout) {
  perKeyLayout = layout;
  const viewport = perKeyQ("#perKeyKeyboardViewport");
  viewport.innerHTML = "";

  const ledKeys = layout.keys.filter((key) => key.ledIndex != null);
  perKeyQ("#perKeyLedCount").textContent = `${ledKeys.length} LEDs`;

  const svg = document.createElementNS(PER_KEY_SVG_NS, "svg");
  svg.setAttribute("class", "keyboard-svg perkey-keyboard-svg");
  svg.setAttribute(
    "viewBox",
    `0 0 ${layout.coordinateSpace.width} ${layout.coordinateSpace.height}`
  );
  svg.setAttribute("preserveAspectRatio", "xMidYMid meet");
  svg.setAttribute("role", "group");
  svg.setAttribute("aria-label", layout.name + " per-key RGB editor");

  for (const key of layout.keys) {
    const group = document.createElementNS(PER_KEY_SVG_NS, "g");
    group.setAttribute(
      "class",
      key.ledIndex == null ? "keyboard-key perkey-key no-led" : "keyboard-key perkey-key"
    );
    group.setAttribute("data-key-id", key.id);
    group.setAttribute("tabindex", key.ledIndex == null ? "-1" : "0");
    group.setAttribute("role", "button");
    group.setAttribute("aria-pressed", String(perKeySelected.has(key.id)));
    if (key.ledIndex == null) group.setAttribute("aria-disabled", "true");
    group.setAttribute(
      "aria-label",
      `${key.label}, LED ${key.ledIndex ?? "unavailable"}, color ${perKeyColorHex(perKeyColors.get(key.id))}`
    );

    if (Number(key.rotation || 0) !== 0) {
      const pivot = key.rotationOrigin || {
        x: key.x + key.width / 2,
        y: key.y + key.height / 2
      };
      group.setAttribute("transform", `rotate(${key.rotation} ${pivot.x} ${pivot.y})`);
    }

    const title = document.createElementNS(PER_KEY_SVG_NS, "title");
    title.textContent =
      `${key.label} | LED ${key.ledIndex ?? "—"} | ${perKeyColorHex(perKeyColors.get(key.id)).toUpperCase()}`;

    const rect = document.createElementNS(PER_KEY_SVG_NS, "rect");
    rect.setAttribute("x", key.x);
    rect.setAttribute("y", key.y);
    rect.setAttribute("width", key.width);
    rect.setAttribute("height", key.height);
    rect.setAttribute("rx", Math.min(4, key.width / 7, key.height / 7));

    const text = document.createElementNS(PER_KEY_SVG_NS, "text");
    text.setAttribute("x", key.x + key.width / 2);
    text.setAttribute("y", key.y + key.height / 2 - 1);
    text.style.fontSize = `${perKeyFontSize(key)}px`;
    text.textContent = key.label;

    const swatch = document.createElementNS(PER_KEY_SVG_NS, "rect");
    const swatchWidth = Math.max(5, Math.min(14, key.width - 7));
    swatch.setAttribute("class", "perkey-led-swatch");
    swatch.setAttribute("x", key.x + (key.width - swatchWidth) / 2);
    swatch.setAttribute("y", key.y + key.height - 5);
    swatch.setAttribute("width", swatchWidth);
    swatch.setAttribute("height", 2.5);
    swatch.setAttribute("rx", "1.25");

    group.append(title, rect, text, swatch);
    group.addEventListener("click", (event) => {
      if (perKeySuppressNextClick) {
        perKeySuppressNextClick = false;
        event.preventDefault();
        event.stopPropagation();
        return;
      }
      if (perKeyRegionMode || key.ledIndex == null) {
        event.preventDefault();
        return;
      }
      perKeyToggleSelection(key, event.ctrlKey || event.metaKey);
    });
    group.addEventListener("keydown", (event) => {
      if ((event.key === "Enter" || event.key === " ") && key.ledIndex != null) {
        event.preventDefault();
        perKeyToggleSelection(key, event.ctrlKey || event.metaKey);
      }
    });
    svg.appendChild(group);
  }

  svg.addEventListener("pointerdown", perKeyRegionPointerDown);
  svg.addEventListener("pointermove", perKeyRegionPointerMove);
  svg.addEventListener("pointerup", perKeyRegionPointerUp);
  svg.addEventListener("pointercancel", perKeyRegionPointerUp);

  viewport.appendChild(svg);
  perKeyPopulateRows();
  perKeyRefreshPreview();
  perKeyRefreshSelection();
}

function perKeySvgPoint(svg, event) {
  const point = svg.createSVGPoint();
  point.x = event.clientX;
  point.y = event.clientY;
  return point.matrixTransform(svg.getScreenCTM().inverse());
}

function perKeyRegionPointerDown(event) {
  if (!perKeyRegionMode) return;
  event.preventDefault();
  const svg = event.currentTarget;
  svg.setPointerCapture(event.pointerId);
  perKeyRegionStart = perKeySvgPoint(svg, event);

  perKeyRegionRect = document.createElementNS(PER_KEY_SVG_NS, "rect");
  perKeyRegionRect.setAttribute("class", "perkey-region-box");
  perKeyRegionRect.setAttribute("x", perKeyRegionStart.x);
  perKeyRegionRect.setAttribute("y", perKeyRegionStart.y);
  perKeyRegionRect.setAttribute("width", "0");
  perKeyRegionRect.setAttribute("height", "0");
  svg.appendChild(perKeyRegionRect);
}

function perKeyRegionPointerMove(event) {
  if (!perKeyRegionMode || !perKeyRegionStart || !perKeyRegionRect) return;
  const point = perKeySvgPoint(event.currentTarget, event);
  const x = Math.min(perKeyRegionStart.x, point.x);
  const y = Math.min(perKeyRegionStart.y, point.y);
  const width = Math.abs(point.x - perKeyRegionStart.x);
  const height = Math.abs(point.y - perKeyRegionStart.y);
  perKeyRegionRect.setAttribute("x", x);
  perKeyRegionRect.setAttribute("y", y);
  perKeyRegionRect.setAttribute("width", width);
  perKeyRegionRect.setAttribute("height", height);
}

function perKeyRotatedCenter(key) {
  const center = { x: key.x + key.width / 2, y: key.y + key.height / 2 };
  const angle = Number(key.rotation || 0);
  if (!angle) return center;
  const pivot = key.rotationOrigin || center;
  const radians = angle * Math.PI / 180;
  const dx = center.x - pivot.x;
  const dy = center.y - pivot.y;
  return {
    x: pivot.x + dx * Math.cos(radians) - dy * Math.sin(radians),
    y: pivot.y + dx * Math.sin(radians) + dy * Math.cos(radians)
  };
}

function perKeyRegionPointerUp(event) {
  if (!perKeyRegionMode || !perKeyRegionStart) return;
  const svg = event.currentTarget;
  const point = perKeySvgPoint(svg, event);
  const minX = Math.min(perKeyRegionStart.x, point.x);
  const maxX = Math.max(perKeyRegionStart.x, point.x);
  const minY = Math.min(perKeyRegionStart.y, point.y);
  const maxY = Math.max(perKeyRegionStart.y, point.y);

  const keys = (perKeyLayout?.keys || []).filter((key) => {
    if (key.ledIndex == null) return false;
    const center = perKeyRotatedCenter(key);
    return center.x >= minX && center.x <= maxX && center.y >= minY && center.y <= maxY;
  });
  perKeySetSelection(keys, event.ctrlKey || event.metaKey);
  perKeySuppressNextClick = true;
  setTimeout(() => {
    perKeySuppressNextClick = false;
  }, 0);

  perKeyRegionRect?.remove();
  perKeyRegionRect = null;
  perKeyRegionStart = null;
  perKeySetRegionMode(false);
}

function perKeySetRegionMode(enabled) {
  perKeyRegionMode = enabled;
  const button = perKeyQ("#regionSelect");
  button.classList.toggle("active", enabled);
  button.setAttribute("aria-pressed", String(enabled));
  perKeyQ("#perKeyKeyboardViewport").classList.toggle("region-mode", enabled);
}

function perKeyPopulateRows() {
  const select = perKeyQ("#perKeyRowSelect");
  const rows = new Map();
  for (const key of perKeyLayout?.keys || []) {
    const grid = key.gridPosition;
    if (!grid) continue;
    const unit = Number(grid.unit ?? 0);
    const row = Number(grid.row);
    rows.set(`${unit}:${row}`, { unit, row });
  }

  const values = [...rows.values()].sort((a, b) => a.unit - b.unit || a.row - b.row);
  const units = new Set(values.map((value) => value.unit));
  select.innerHTML = "";
  for (const value of values) {
    const option = document.createElement("option");
    option.value = `${value.unit}:${value.row}`;
    option.textContent = units.size > 1
      ? `Unit ${value.unit + 1} · Row ${value.row + 1}`
      : `Row ${value.row + 1}`;
    select.appendChild(option);
  }
}

function perKeySelectRow() {
  const [unit, row] = perKeyQ("#perKeyRowSelect").value.split(":").map(Number);
  const keys = perKeyLayout.keys.filter((key) =>
    key.ledIndex != null &&
    Number(key.gridPosition?.unit ?? 0) === unit &&
    Number(key.gridPosition?.row) === row
  );
  perKeySetSelection(keys);
}

function perKeySelectWasd() {
  const usages = new Set([0x1A, 0x04, 0x16, 0x07]);
  perKeySetSelection(
    perKeyLayout.keys.filter(
      (key) => key.ledIndex != null && usages.has(Number(key.hidUsage))
    )
  );
}

function perKeySelectNumpad() {
  perKeySetSelection(
    perKeyLayout.keys.filter((key) => key.ledIndex != null && key.section === "numpad")
  );
}

function perKeyApplyColor(color) {
  const keys = perKeyOrderedSelection();
  if (keys.length === 0) return;
  for (const key of keys) {
    if (perKeyIsBlack(color)) perKeyColors.delete(key.id);
    else perKeyColors.set(key.id, { ...color });
  }
  perKeyRefreshPreview();
  perKeyUpdateDirty();
}

function perKeyInterpolateColor(start, end, position) {
  const mix = (a, b) => Math.round(a + (b - a) * position);
  return {
    r: mix(start.r, end.r),
    g: mix(start.g, end.g),
    b: mix(start.b, end.b)
  };
}

function perKeyApplyGradient() {
  const keys = perKeyOrderedSelection();
  if (keys.length < 2) return;
  const start = perKeyHexColor(perKeyQ("#perKeyGradientStart").value);
  const end = perKeyHexColor(perKeyQ("#perKeyGradientEnd").value);

  keys.forEach((key, index) => {
    const position = index / (keys.length - 1);
    perKeyColors.set(key.id, perKeyInterpolateColor(start, end, position));
  });
  perKeyRefreshPreview();
  perKeyUpdateDirty();
}

function perKeyCopy() {
  const keys = perKeyOrderedSelection();
  if (keys.length === 0) return;
  perKeyClipboard = keys.map((key) => ({ ...(perKeyColors.get(key.id) || { r: 0, g: 0, b: 0 }) }));
  perKeyQ("#pastePerKeyColor").disabled = false;
  perKeyUpdateDirty(
    `Copied ${perKeyClipboard.length} key color${perKeyClipboard.length === 1 ? "" : "s"}.`
  );
}

function perKeyPaste() {
  const keys = perKeyOrderedSelection();
  if (!perKeyClipboard || keys.length === 0) return;

  if (perKeyClipboard.length !== 1 && perKeyClipboard.length !== keys.length) {
    perKeyUpdateDirty(
      `Paste requires one copied color or matching selections. Copied ${perKeyClipboard.length}, selected ${keys.length}.`
    );
    return;
  }

  keys.forEach((key, index) => {
    const color = perKeyClipboard.length === 1 ? perKeyClipboard[0] : perKeyClipboard[index];
    if (perKeyIsBlack(color)) perKeyColors.delete(key.id);
    else perKeyColors.set(key.id, { ...color });
  });
  perKeyRefreshPreview();
  perKeyUpdateDirty();
}

async function perKeySave() {
  const button = perKeyQ("#savePerKeyRgb");
  button.disabled = true;
  perKeyUpdateDirty("Validating and writing the complete stored RGB table...");

  const colors = [...perKeyColors.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([keyId, color]) => ({ keyId, color }));

  try {
    const result = await perKeyInvoke("save_per_key_rgb", {
      brightness: Number(perKeyQ("#perKeyBrightness").value),
      colors
    });
    perKeySavedSnapshot = perKeySnapshot();
    perKeyUpdateDirty(
      result.applied
        ? `Saved and applied ${result.colorCount} lit key${result.colorCount === 1 ? "" : "s"}. Physical LED verification is still required.`
        : `Desired per-key RGB saved locally. Keyboard not applied: ${result.message || "not ready"}`
    );
  } catch (error) {
    perKeyUpdateDirty(`Stored RGB rejected: ${String(error)}`);
  } finally {
    button.disabled = false;
  }
}

async function perKeyReadHardware() {
  const button = perKeyQ("#readPerKeyHardware");
  button.disabled = true;
  perKeyUpdateDirty("Reading 04 F5 RGB state from the keyboard...");
  try {
    const result = await perKeyInvoke("read_per_key_rgb");
    perKeyColors.clear();
    for (const keyColor of result.colors || []) {
      if (!perKeyIsBlack(keyColor.color)) {
        perKeyColors.set(keyColor.keyId, keyColor.color);
      }
    }
    perKeyRefreshPreview();
    perKeyUpdateDirty(
      `Read ${result.colors?.length || 0} physical key colors from the keyboard. Save to make this the desired state.`
    );
  } catch (error) {
    perKeyUpdateDirty(`Could not read keyboard colors: ${String(error)}`);
  } finally {
    button.disabled = false;
  }
}

function perKeyBindEvents() {
  perKeyQ("#selectPerKeyRow").addEventListener("click", perKeySelectRow);
  perKeyQ("#selectWasd").addEventListener("click", perKeySelectWasd);
  perKeyQ("#selectNumpad").addEventListener("click", perKeySelectNumpad);
  perKeyQ("#regionSelect").addEventListener("click", () => perKeySetRegionMode(!perKeyRegionMode));
  perKeyQ("#clearPerKeySelection").addEventListener("click", () => perKeySetSelection([]));
  perKeyQ("#applyPerKeyColor").addEventListener("click", () => {
    perKeyApplyColor(perKeyHexColor(perKeyQ("#perKeyColor").value));
  });
  perKeyQ("#applyPerKeyGradient").addEventListener("click", perKeyApplyGradient);
  perKeyQ("#copyPerKeyColor").addEventListener("click", perKeyCopy);
  perKeyQ("#pastePerKeyColor").addEventListener("click", perKeyPaste);
  perKeyQ("#clearPerKeyColor").addEventListener("click", () => perKeyApplyColor({ r: 0, g: 0, b: 0 }));
  perKeyQ("#savePerKeyRgb").addEventListener("click", () => void perKeySave());
  perKeyQ("#readPerKeyHardware").addEventListener("click", () => void perKeyReadHardware());

  perKeyQ("#perKeyBrightness").addEventListener("input", (event) => {
    perKeyQ("#perKeyBrightnessOut").value = event.target.value;
    perKeyUpdateDirty();
  });

  document.querySelectorAll("#perKeyPalette [data-color]").forEach((button) => {
    button.addEventListener("click", () => {
      perKeyQ("#perKeyColor").value = button.dataset.color;
      perKeyApplyColor(perKeyHexColor(button.dataset.color));
    });
  });

  window.addEventListener("mechkeymesh-layout-changed", (event) => {
    const layout = event.detail?.layout;
    if (!layout || layout.id === perKeyLayout?.id) return;
    perKeySelected.clear();
    perKeyRender(layout);
  });
}

async function initializePerKeyRgbEditor() {
  if (perKeyInitialized) return;
  perKeyInitialized = true;
  perKeyBindEvents();

  try {
    const [layout, capabilities, desired] = await Promise.all([
      perKeyInvoke("get_primary_layout"),
      perKeyInvoke("get_per_key_rgb_capabilities"),
      perKeyInvoke("get_desired_per_key_rgb")
    ]);

    perKeyLayout = layout;
    perKeyCapabilities = capabilities;
    perKeyQ("#perKeyBrightness").min = String(capabilities.brightnessMin);
    perKeyQ("#perKeyBrightness").max = String(capabilities.brightnessMax);
    perKeyQ("#perKeyBrightness").value = String(desired.brightness);
    perKeyQ("#perKeyBrightnessOut").value = String(desired.brightness);
    perKeyQ("#readPerKeyHardware").disabled = !capabilities.readbackSupported;

    perKeyColors = new Map(
      (desired.colors || [])
        .filter((keyColor) => !perKeyIsBlack(keyColor.color))
        .map((keyColor) => [keyColor.keyId, keyColor.color])
    );
    perKeySavedSnapshot = perKeySnapshot();
    perKeyRender(layout);
    perKeyUpdateDirty();
  } catch (error) {
    perKeyQ("#perKeyStatus").textContent =
      `Could not initialize stored per-key RGB: ${String(error)}`;
    perKeyQ("#savePerKeyRgb").disabled = true;
    perKeyQ("#readPerKeyHardware").disabled = true;
  }
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initializePerKeyRgbEditor, { once: true });
} else {
  initializePerKeyRgbEditor();
}
