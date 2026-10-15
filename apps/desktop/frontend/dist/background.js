(() => {
  const invoke = (command, args = {}) => window.__TAURI__.core.invoke(command, args);
  const byId = (id) => document.getElementById(id);
  function message(value, error = false) {
    byId("backgroundMessage").textContent = String(value);
    byId("backgroundMessage").classList.toggle("error-text", error);
  }
  let paused = false;
  async function refresh(loadPreferences = false) {
    try {
      const status = await invoke("get_background_status");
      paused = status.paused;
      if (loadPreferences) {
        byId("minimizeToTray").checked = status.preferences.minimizeToTray;
        byId("startMinimized").checked = status.preferences.startMinimized;
        byId("startWithWindows").checked = status.startWithWindows;
      }
      byId("pauseEffects").textContent = paused ? "Resume live effects" : "Pause live effects";
      byId("backgroundRuntime").textContent = status.sleeping ? "Windows sleeping: keyboard I/O paused" : paused ? "Live effects paused" : "Background operation enabled";
    } catch (error) { message(error, true); }
  }
  byId("saveBackgroundPreferences").addEventListener("click", async () => {
    try {
      await invoke("save_background_preferences", {
        preferences: { minimizeToTray: byId("minimizeToTray").checked, startMinimized: byId("startMinimized").checked },
        startWithWindows: byId("startWithWindows").checked
      });
      message("Background preferences saved.");
      await refresh(true);
    } catch (error) { message(error, true); }
  });
  byId("pauseEffects").addEventListener("click", async () => {
    await invoke("pause_background_effects", { paused: !paused });
    await refresh();
  });
  void refresh(true);
  setInterval(refresh, 2000);
})();

