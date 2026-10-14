const liveRgbEffects = [
  ["static", "Static"],
  ["key-highlight", "Key highlight"],
  ["ripple", "Ripple"],
  ["meteor", "Meteor"],
  ["lightning", "Lightning"],
  ["wave", "Wave"],
  ["trail", "Trail"],
  ["breathing", "Breathing"],
  ["gradient", "Gradient"],
  ["rain", "Rain"],
  ["sparkle", "Sparkle"]
];

const liveRgbKinds = [
  ["static", "Static"],
  ["animation", "Animation"],
  ["reactive", "Reactive"],
  ["audio", "Audio"]
];

const liveRgbBlends = [
  ["replace", "Replace"],
  ["add", "Add"],
  ["multiply", "Multiply"],
  ["screen", "Screen"]
];

let liveRgbScene = null;
let liveRgbLayoutId = null;

function liveRgbEscape(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function liveRgbOptions(options, selected) {
  return options.map(([value, label]) =>
    `<option value="${value}"${value === selected ? " selected" : ""}>${label}</option>`
  ).join("");
}

function liveRgbHex(color) {
  const byte = (value) => Number(value || 0).toString(16).padStart(2, "0");
  return `#${byte(color?.r)}${byte(color?.g)}${byte(color?.b)}`;
}

function liveRgbColor(hex) {
  const value = hex.replace("#", "");
  return {
    r: parseInt(value.slice(0, 2), 16),
    g: parseInt(value.slice(2, 4), 16),
    b: parseInt(value.slice(4, 6), 16)
  };
}

function newLiveRgbLayer() {
  return {
    id: `layer-${Date.now()}-${Math.floor(Math.random() * 10000)}`,
    name: "New layer",
    kind: "animation",
    effect: "wave",
    enabled: true,
    opacity: 1,
    blendMode: "replace",
    mask: [],
    order: liveRgbScene?.layers?.length || 0,
    primary: { r: 33, g: 230, b: 232 },
    secondary: { r: 28, g: 141, b: 255 },
    speed: 1,
    width: 1
  };
}

function renderLiveRgbLayers() {
  if (!liveRgbScene) return;
  $("#liveRgbLayers").innerHTML = liveRgbScene.layers.map((layer, index) => `
    <article class="live-rgb-layer" data-index="${index}">
      <div class="live-rgb-layer-group">
        <label class="live-rgb-layer-inline"><input class="layer-enabled" type="checkbox"${layer.enabled ? " checked" : ""}/> Enabled</label>
        <label>Name<input class="layer-name" type="text" value="${liveRgbEscape(layer.name)}" /></label>
        <label>Layer type<select class="layer-kind">${liveRgbOptions(liveRgbKinds, layer.kind)}</select></label>
      </div>
      <div class="live-rgb-layer-group">
        <label>Effect<select class="layer-effect">${liveRgbOptions(liveRgbEffects, layer.effect)}</select></label>
        <label>Blend<select class="layer-blend">${liveRgbOptions(liveRgbBlends, layer.blendMode)}</select></label>
        <label>Opacity<input class="layer-opacity" type="range" min="0" max="100" value="${Math.round(layer.opacity * 100)}" /></label>
      </div>
      <div class="live-rgb-layer-group">
        <div class="live-rgb-layer-inline">
          <label>Primary<input class="layer-primary live-rgb-color" type="color" value="${liveRgbHex(layer.primary)}" /></label>
          <label>Secondary<input class="layer-secondary live-rgb-color" type="color" value="${liveRgbHex(layer.secondary)}" /></label>
        </div>
        <label>Speed<input class="layer-speed" type="number" min="0.05" max="20" step="0.05" value="${layer.speed}" /></label>
        <label>Width<input class="layer-width" type="number" min="0.05" max="10" step="0.05" value="${layer.width}" /></label>
      </div>
      <div class="live-rgb-layer-group">
        <label>Physical key IDs<input class="layer-mask" type="text" value="${liveRgbEscape((layer.mask || []).join(", "))}" placeholder="All keys" /></label>
        <span class="muted">Blank selects all keys.</span>
      </div>
      <div class="live-rgb-order">
        <button class="secondary compact move-layer-up" type="button" title="Move up">↑</button>
        <button class="secondary compact move-layer-down" type="button" title="Move down">↓</button>
        <button class="secondary compact duplicate-layer" type="button">Duplicate</button>
        <button class="secondary compact delete-layer" type="button">Delete</button>
      </div>
    </article>
  `).join("");
}

function collectLiveRgbScene() {
  if (!liveRgbScene) return null;
  const layers = [...document.querySelectorAll(".live-rgb-layer")].map((element, order) => ({
    id: liveRgbScene.layers[Number(element.dataset.index)]?.id || `layer-${order}`,
    name: element.querySelector(".layer-name").value.trim(),
    kind: element.querySelector(".layer-kind").value,
    effect: element.querySelector(".layer-effect").value,
    enabled: element.querySelector(".layer-enabled").checked,
    opacity: Number(element.querySelector(".layer-opacity").value) / 100,
    blendMode: element.querySelector(".layer-blend").value,
    mask: element.querySelector(".layer-mask").value.split(",").map((value) => value.trim()).filter(Boolean),
    order,
    primary: liveRgbColor(element.querySelector(".layer-primary").value),
    secondary: liveRgbColor(element.querySelector(".layer-secondary").value),
    speed: Number(element.querySelector(".layer-speed").value),
    width: Number(element.querySelector(".layer-width").value)
  }));
  liveRgbScene = {
    name: $("#liveRgbSceneName").value.trim(),
    framesPerSecond: Number($("#liveRgbFps").value),
    brightness: Number($("#liveRgbBrightness").value) / 100,
    layers
  };
  return liveRgbScene;
}

function populateLiveRgbPreviewKeys() {
  if (!activeLayout || activeLayout.id === liveRgbLayoutId) return;
  liveRgbLayoutId = activeLayout.id;
  $("#liveRgbPreviewKey").innerHTML = activeLayout.keys
    .map((key) => `<option value="${liveRgbEscape(key.id)}">${liveRgbEscape(key.label)} · ${liveRgbEscape(key.id)}</option>`)
    .join("");
}

async function loadLiveRgb() {
  try {
    const stored = localStorage.getItem("mechkeymesh-live-rgb-scene");
    liveRgbScene = stored ? JSON.parse(stored) : await invoke("get_live_rgb_defaults");
    $("#liveRgbSceneName").value = liveRgbScene.name;
    $("#liveRgbFps").value = String(liveRgbScene.framesPerSecond);
    $("#liveRgbBrightness").value = String(Math.round(liveRgbScene.brightness * 100));
    $("#liveRgbBrightnessOut").value = `${Math.round(liveRgbScene.brightness * 100)}%`;
    renderLiveRgbLayers();
    $("#liveRgbStatus").textContent = "Scene ready. Streaming is available on the verified USB path only.";
  } catch (error) {
    $("#liveRgbStatus").textContent = String(error);
  }
}

$("#liveRgbBrightness").addEventListener("input", (event) => {
  $("#liveRgbBrightnessOut").value = `${event.target.value}%`;
});

$("#addLiveRgbLayer").addEventListener("click", () => {
  collectLiveRgbScene();
  liveRgbScene.layers.push(newLiveRgbLayer());
  renderLiveRgbLayers();
});

$("#liveRgbLayers").addEventListener("click", (event) => {
  const row = event.target.closest(".live-rgb-layer");
  if (!row) return;
  const index = Number(row.dataset.index);
  collectLiveRgbScene();
  if (event.target.closest(".move-layer-up") && index > 0) {
    [liveRgbScene.layers[index - 1], liveRgbScene.layers[index]] = [liveRgbScene.layers[index], liveRgbScene.layers[index - 1]];
  } else if (event.target.closest(".move-layer-down") && index < liveRgbScene.layers.length - 1) {
    [liveRgbScene.layers[index + 1], liveRgbScene.layers[index]] = [liveRgbScene.layers[index], liveRgbScene.layers[index + 1]];
  } else if (event.target.closest(".duplicate-layer")) {
    const copy = structuredClone(liveRgbScene.layers[index]);
    copy.id = `layer-${Date.now()}-${Math.floor(Math.random() * 10000)}`;
    copy.name = `${copy.name} copy`;
    liveRgbScene.layers.splice(index + 1, 0, copy);
  } else if (event.target.closest(".delete-layer")) {
    liveRgbScene.layers.splice(index, 1);
  } else {
    return;
  }
  liveRgbScene.layers.forEach((layer, order) => { layer.order = order; });
  renderLiveRgbLayers();
});

$("#startLiveRgb").addEventListener("click", async () => {
  const scene = collectLiveRgbScene();
  localStorage.setItem("mechkeymesh-live-rgb-scene", JSON.stringify(scene));
  $("#liveRgbStatus").textContent = "Starting renderer...";
  try {
    await invoke("start_live_rgb", { scene });
    $("#liveRgbStatus").textContent = "Rendering in the background. The latest-frame queue is capped at one frame.";
  } catch (error) {
    $("#liveRgbStatus").textContent = String(error);
  }
});

$("#stopLiveRgb").addEventListener("click", async () => {
  await invoke("stop_live_rgb");
  $("#liveRgbStatus").textContent = "Live RGB stopped. Stored lighting was not changed.";
});

$("#previewLiveRgbKey").addEventListener("click", async () => {
  const keyId = $("#liveRgbPreviewKey").value;
  if (!keyId) return;
  try {
    await invoke("preview_live_rgb_key", { keyId });
  } catch (error) {
    $("#liveRgbStatus").textContent = String(error);
  }
});

async function pollLiveRgbStatus() {
  populateLiveRgbPreviewKeys();
  try {
    const status = await invoke("get_live_rgb_status");
    $("#liveRgbRunState").textContent = status.running ? "Running" : "Stopped";
    $("#liveRgbRunState").classList.toggle("pending", !status.running);
    $("#liveRgbRunState").classList.toggle("verified", status.running);
    const transport = status.transport;
    $("#liveRgbTransportSummary").textContent =
      `Bounded queue ${transport.bufferedFrames}/1 · ${transport.sentFrames} sent · ${transport.coalescedFrames} coalesced · ${transport.failedFrames} failed`;
    if (status.lastError || transport.lastError) $("#liveRgbStatus").textContent = status.lastError || transport.lastError;
  } catch (error) {
    $("#liveRgbStatus").textContent = String(error);
  }
}

void loadLiveRgb();
setInterval(pollLiveRgbStatus, 1000);
