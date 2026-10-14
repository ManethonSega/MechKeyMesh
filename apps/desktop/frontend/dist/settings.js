const settingsInvoke = (command, args = {}) => window.__TAURI__.core.invoke(command, args);
const settingsQ = (selector) => document.querySelector(selector);

let settingsDescriptor = null;

function settingsStatus(message, isError = false) {
  const element = settingsQ("#deviceSettingsStatus");
  element.textContent = message;
  element.style.color = isError ? "#ff718c" : "";
}

function option(value, label) {
  const element = document.createElement("option");
  element.value = value;
  element.textContent = label;
  return element;
}

function populateSettingsDescriptor(descriptor) {
  settingsDescriptor = descriptor;
  const sleep = settingsQ("#sleepTimeout");
  sleep.innerHTML = "";
  descriptor.sleepOptions.forEach((item) => {
    sleep.appendChild(option(item.timeout, item.label));
  });

  const fnMode = settingsQ("#deviceFnMode");
  fnMode.innerHTML = "";
  descriptor.fnModes.forEach((item) => {
    const label = item.mode === "momentary" ? "Momentary (hold Fn)" : "Toggle (press Fn)";
    fnMode.appendChild(option(item.mode, label));
  });

  const response = settingsQ("#keyResponseLevel");
  response.innerHTML = "";
  descriptor.keyResponseLevels.forEach((item) => {
    response.appendChild(option(String(item.level), `Level ${item.level} · ${item.label}`));
  });

  const body = settingsQ("#responseTimingRows");
  body.innerHTML = "";
  descriptor.keyResponseLevels.forEach((item) => {
    const row = document.createElement("tr");
    for (const value of [
      `Level ${item.level}`,
      item.label,
      item.wiredTiming,
      item.wirelessTiming,
      item.bluetoothTiming
    ]) {
      const cell = document.createElement("td");
      cell.textContent = value;
      row.appendChild(cell);
    }
    body.appendChild(row);
  });

  settingsQ("#settingsEvidenceNote").textContent = descriptor.note;
}

function renderSettings(settings) {
  settingsQ("#disableWindowsKey").checked = settings.disableWindowsKey;
  settingsQ("#disableAltTab").checked = settings.disableAltTab;
  settingsQ("#disableAltF4").checked = settings.disableAltF4;
  settingsQ("#deviceFnMode").value = settings.fnMode;
  settingsQ("#sleepTimeout").value = settings.sleepTimeout;
  settingsQ("#keyResponseLevel").value = String(settings.keyResponseLevel);
}

function settingsFromEditor() {
  return {
    disableWindowsKey: settingsQ("#disableWindowsKey").checked,
    disableAltTab: settingsQ("#disableAltTab").checked,
    disableAltF4: settingsQ("#disableAltF4").checked,
    fnMode: settingsQ("#deviceFnMode").value,
    sleepTimeout: settingsQ("#sleepTimeout").value,
    keyResponseLevel: Number(settingsQ("#keyResponseLevel").value)
  };
}

async function loadDeviceSettings() {
  try {
    const [descriptor, desired] = await Promise.all([
      settingsInvoke("get_device_settings_descriptor"),
      settingsInvoke("get_desired_device_settings")
    ]);
    populateSettingsDescriptor(descriptor);
    if (desired) {
      renderSettings(desired);
      settingsStatus("Loaded the last desired settings. Hardware state is write-only and was not queried.");
    } else {
      renderSettings({
        disableWindowsKey: false,
        disableAltTab: false,
        disableAltF4: false,
        fnMode: "momentary",
        sleepTimeout: "five-minutes",
        keyResponseLevel: 3
      });
      settingsStatus("No desired settings are saved yet. Review every value before applying.");
    }
  } catch (error) {
    settingsStatus(String(error), true);
    settingsQ("#applyDeviceSettings").disabled = true;
  }
}

async function applyDeviceSettings() {
  if (!settingsDescriptor) return;
  const button = settingsQ("#applyDeviceSettings");
  button.disabled = true;
  settingsStatus("Saving and applying the complete settings block...");
  try {
    const result = await settingsInvoke("save_device_settings", {
      settings: settingsFromEditor()
    });
    if (result.applied) {
      settingsStatus("Desired settings saved and sent. Physical change/verify/restore testing remains pending.");
    } else {
      settingsStatus(`Desired settings saved, but hardware apply failed: ${result.message}`, true);
    }
  } catch (error) {
    settingsStatus(String(error), true);
  } finally {
    button.disabled = false;
  }
}

settingsQ("#applyDeviceSettings").addEventListener("click", applyDeviceSettings);
void loadDeviceSettings();
