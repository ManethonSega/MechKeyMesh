const $ = (selector) => document.querySelector(selector);
const invoke = (command, args = {}) => window.__TAURI__.core.invoke(command, args);

const pages = {
  home: ["Home", "Device overview and connection health"],
  lighting: ["Lighting", "Verified onboard effects for the active keyboard"],
  diagnostics: ["Diagnostics", "Connection, command and recovery log"]
};

function showPage(name) {
  document.querySelectorAll(".page").forEach((page) => page.classList.remove("active"));
  document.querySelectorAll("#nav button").forEach((button) => button.classList.toggle("active", button.dataset.page === name));
  document.querySelector("#page-" + name)?.classList.add("active");
  $("#pageTitle").textContent = pages[name]?.[0] || name;
  $("#pageSubtitle").textContent = pages[name]?.[1] || "";
  if (name === "diagnostics") refreshLogs();
}

document.querySelectorAll("#nav button:not(:disabled)").forEach((button) => {
  button.addEventListener("click", () => showPage(button.dataset.page));
});

$("#brightness").addEventListener("input", (event) => $("#brightnessOut").value = event.target.value);
$("#speed").addEventListener("input", (event) => $("#speedOut").value = event.target.value);

async function loadModes() {
  const modes = await invoke("get_lighting_modes");
  $("#lightingMode").innerHTML = modes
    .map((mode, index) => `<option value="${index}">${index}. ${mode}</option>`)
    .join("");
}

function stateClass(state) {
  return ["ready", "busy", "recovery", "unsupported"].includes(state) ? state : "disconnected";
}

function updateStatus(status) {
  const state = status?.state || "disconnected";
  const device = status?.device || null;
  const pill = $("#connectionPill");
  pill.textContent = state;
  pill.className = "pill " + stateClass(state);

  $("#deviceName").textContent = device?.name || "AULA F108 Pro";
  $("#deviceDetail").textContent =
    status?.lastError ||
    (state === "ready" ? "Configuration interface ready" : "Waiting for a supported keyboard...");
  $("#connectionType").textContent = device?.connection || "—";
  $("#driverId").textContent = device?.driverId || "aula.f108pro.usb";
  $("#vidpid").textContent =
    device?.vendorId != null
      ? device.vendorId.toString(16).padStart(4, "0").toUpperCase() +
        " / " +
        device.productId.toString(16).padStart(4, "0").toUpperCase()
      : "—";

  $("#applyLighting").disabled = state !== "ready";
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
    await invoke("apply_lighting", { config });
    $("#lightingResult").textContent = "Applied successfully.";
  } catch (error) {
    $("#lightingResult").textContent = "Failed: " + error;
  }
  await pollStatus();
});

$("#reconnectButton").addEventListener("click", async () => {
  await invoke("reconnect");
  setTimeout(pollStatus, 250);
});

async function refreshLogs() {
  try {
    const logs = await invoke("get_logs");
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
  await loadModes();
  await pollStatus();
  setTimeout(() => {
    $("#splash").classList.add("hidden");
    $("#app").classList.remove("hidden");
  }, 850);
  setInterval(pollStatus, 1200);
})();
