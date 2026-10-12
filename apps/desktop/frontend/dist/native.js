(() => {
  const field = (id, value) => { document.getElementById(id).textContent = value; };
  async function refresh() {
    try {
      const data = await window.__TAURI__.core.invoke("get_native_observations");
      field("nativeConnection", data.connection || "Disconnected");
      field("nativeBattery", data.batteryPercent == null ? "Not exposed on this connection" : `${data.batteryPercent}%`);
      field("nativeCharging", data.charging == null ? "Not verified / unavailable" : data.charging ? "Charging" : "Not charging");
      field("nativeFirmware", data.firmware || "Not verified / unavailable");
      field("nativeRevision", data.hardwareRevision || "Not verified / unavailable");
      field("nativeKnob", data.knobPolicy);
      field("nativeEvidence", data.evidenceNote);
    } catch (_) {}
  }
  void refresh();
  setInterval(refresh, 5000);
})();
