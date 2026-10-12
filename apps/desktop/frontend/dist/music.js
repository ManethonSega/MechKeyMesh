let musicConfig = null;

function musicHex(color) {
  const byte = (value) => Number(value || 0).toString(16).padStart(2, "0");
  return `#${byte(color?.r)}${byte(color?.g)}${byte(color?.b)}`;
}

function musicColor(hex) {
  const value = hex.replace("#", "");
  return {
    r: parseInt(value.slice(0, 2), 16),
    g: parseInt(value.slice(2, 4), 16),
    b: parseInt(value.slice(4, 6), 16)
  };
}

function setMusicControls(config) {
  $("#musicOutputDevice").value = config.outputDeviceId || "";
  $("#musicMode").value = config.mode;
  $("#musicBackground").value = config.backgroundMode;
  $("#musicSensitivity").value = String(Math.round(config.sensitivity * 100));
  $("#musicSmoothing").value = String(Math.round(config.smoothing * 100));
  $("#musicFalloff").value = String(Math.round(config.falloff * 100));
  $("#musicBass").value = String(Math.round(config.bass * 100));
  $("#musicMids").value = String(Math.round(config.mids * 100));
  $("#musicTreble").value = String(Math.round(config.treble * 100));
  $("#musicBrightness").value = String(Math.round(config.brightness * 100));
  [...document.querySelectorAll("#musicPalette input")].forEach((input, index) => {
    if (config.palette[index]) input.value = musicHex(config.palette[index]);
  });
  updateMusicOutputs();
}

function collectMusicConfig() {
  musicConfig = {
    outputDeviceId: $("#musicOutputDevice").value || null,
    mode: $("#musicMode").value,
    backgroundMode: $("#musicBackground").value,
    sensitivity: Number($("#musicSensitivity").value) / 100,
    smoothing: Number($("#musicSmoothing").value) / 100,
    falloff: Number($("#musicFalloff").value) / 100,
    bass: Number($("#musicBass").value) / 100,
    mids: Number($("#musicMids").value) / 100,
    treble: Number($("#musicTreble").value) / 100,
    brightness: Number($("#musicBrightness").value) / 100,
    palette: [...document.querySelectorAll("#musicPalette input")].map((input) => musicColor(input.value))
  };
  return musicConfig;
}

function updateMusicOutputs() {
  for (const name of ["Sensitivity", "Bass", "Mids", "Treble"]) {
    const value = Number($("#music" + name).value) / 100;
    $("#music" + name + "Out").value = `${value.toFixed(2)}×`;
  }
  for (const name of ["Smoothing", "Falloff", "Brightness"]) {
    $("#music" + name + "Out").value = `${$("#music" + name).value}%`;
  }
}

async function loadMusic() {
  try {
    const [defaults, devices] = await Promise.all([
      invoke("get_music_defaults"),
      invoke("get_audio_output_devices")
    ]);
    const stored = localStorage.getItem("mechkeymesh-music-config");
    musicConfig = stored ? { ...defaults, ...JSON.parse(stored) } : defaults;
    $("#musicOutputDevice").innerHTML = '<option value="">System default</option>' + devices
      .map((device) => `<option value="${liveRgbEscape(device.id)}">${liveRgbEscape(device.name)}${device.isDefault ? " · Default" : ""}</option>`)
      .join("");
    setMusicControls(musicConfig);
    $("#musicStatus").textContent = devices.length
      ? `${devices.length} output device${devices.length === 1 ? "" : "s"} available.`
      : "Windows output devices are available when the desktop app runs on Windows.";
  } catch (error) {
    $("#musicStatus").textContent = String(error);
  }
}

for (const id of [
  "musicSensitivity", "musicSmoothing", "musicFalloff", "musicBass", "musicMids",
  "musicTreble", "musicBrightness"
]) {
  $("#" + id).addEventListener("input", updateMusicOutputs);
}

$("#startMusic").addEventListener("click", async () => {
  const config = collectMusicConfig();
  localStorage.setItem("mechkeymesh-music-config", JSON.stringify(config));
  $("#musicStatus").textContent = "Starting output-loopback capture...";
  try {
    await invoke("start_music_lighting", { config });
    $("#musicStatus").textContent = "Listening to Windows output. Audio never leaves memory.";
  } catch (error) {
    $("#musicStatus").textContent = String(error);
  }
});

$("#stopMusic").addEventListener("click", async () => {
  await invoke("stop_music_lighting");
  $("#musicStatus").textContent = "Music lighting stopped. Stored lighting was not changed.";
});

function renderMusicSpectrum(spectrum) {
  const bands = spectrum?.bands || Array(16).fill(0);
  $("#musicSpectrumBars").innerHTML = bands
    .map((level) => `<i style="--level:${Math.max(0.02, Math.min(1, Number(level) || 0))}"></i>`)
    .join("");
  const percent = (value) => `${Math.round((Number(value) || 0) * 100)}%`;
  $("#musicBandSummary").textContent =
    `Bass ${percent(spectrum?.bass)} · Mids ${percent(spectrum?.mids)} · Treble ${percent(spectrum?.treble)}`;
}

async function pollMusicStatus() {
  try {
    const status = await invoke("get_music_status");
    $("#musicRunState").textContent = status.running ? "Running" : "Stopped";
    $("#musicRunState").classList.toggle("pending", !status.running);
    $("#musicRunState").classList.toggle("verified", status.running);
    renderMusicSpectrum(status.spectrum);
    $("#musicTransportSummary").textContent =
      `${status.framesAnalyzed} analyses · ${status.framesRendered} rendered · ${status.transport.coalescedFrames} transport coalesced`;
    if (status.lastError) $("#musicStatus").textContent = status.lastError;
  } catch (error) {
    $("#musicStatus").textContent = String(error);
  }
}

renderMusicSpectrum(null);
void loadMusic();
setInterval(pollMusicStatus, 250);
