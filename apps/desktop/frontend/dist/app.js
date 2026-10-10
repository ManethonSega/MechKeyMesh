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
  if (["ready"].includes(state)) return "ready";
  if (["busy", "detecting", "opening", "initializing"].includes(state)) return "busy";
  if (["recovery", "ambiguous"].includes(state)) return "recovery";
  if (state === "unsupported") return "unsupported";
  return "disconnected";
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
          return `${driver.driverId} | ${driver.deviceName} | ${matches}`;
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
  await loadModes();
  await loadPersistence();
  await loadDesiredLighting();
  await pollStatus();
  setTimeout(() => {
    $("#splash").classList.add("hidden");
    $("#app").classList.remove("hidden");
  }, 850);
  setInterval(pollStatus, 1200);
})();
