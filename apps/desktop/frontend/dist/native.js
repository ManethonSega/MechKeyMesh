(() => {
  const field = (id, value) => { document.getElementById(id).textContent = value; };
  async function refresh() {
    try {
      const data = await window.__TAURI__.core.invoke("get_native_observations");
      field("nativeConnection", data.connection || "Disconnected");
      field("nativeBattery", data.batteryPercent == null ? "Not exposed on this connection" : `${data.batteryPercent}%`);
      const charging = !data.connection ? "Disconnected" : typeof data.charging !== "boolean" ? "Not reported" : data.charging ? "Charging" : "Not charging";
      field("nativeCharging", charging);
      field("homeCharging", charging);
      field("nativeFirmware", data.firmware || "Not verified / unavailable");
      field("nativeRevision", data.hardwareRevision || "Not verified / unavailable");
      field("nativeKnob", data.knobPolicy);
      field("nativeEvidence", data.evidenceNote);
    } catch (_) {
      field("nativeCharging", "Unavailable");
      field("homeCharging", "Unavailable");
    }
  }
  void refresh();
  setInterval(refresh, 5000);
})();
