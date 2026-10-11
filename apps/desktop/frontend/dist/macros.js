const macroInvoke = (command, args = {}) => window.__TAURI__.core.invoke(command, args);
const macroQ = (selector) => document.querySelector(selector);

let macroPrograms = [];
let macroDraft = null;
let macroSelectedEvents = new Set();
let macroEventClipboard = [];
let macroRecording = false;
let macroRecordLast = 0;

function macroId() {
  if (globalThis.crypto?.randomUUID) return globalThis.crypto.randomUUID();
  return `macro-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function macroClone(value) {
  return JSON.parse(JSON.stringify(value));
}

function newMacroProgram(name = "New macro") {
  return { id: macroId(), name, playback: { mode: "once" }, events: [] };
}

function macroSetStatus(message, isError = false) {
  const status = macroQ("#macroStatus");
  status.textContent = message;
  status.style.color = isError ? "#ff718c" : "";
}

function macroMarkDirty(message = "Unsaved macro changes") {
  macroSetStatus(message);
}

function macroEventName(event) {
  return ({
    "key-down": "Key down",
    "key-up": "Key up",
    "mouse-down": "Mouse down",
    "mouse-up": "Mouse up",
    "delay": "Delay"
  })[event.type] || event.type;
}

function macroMouseLabel(button) {
  return ({ left: "Mouse 1", right: "Mouse 2", middle: "Mouse 3", button4: "Mouse 4", button5: "Mouse 5" })[button] || button;
}

function macroUsageLabel(usage) {
  const number = Number(usage);
  if (number >= 4 && number <= 29) return String.fromCharCode(65 + number - 4);
  if (number >= 30 && number <= 38) return String(number - 29);
  if (number === 39) return "0";
  return ({ 40: "Enter", 41: "Escape", 42: "Backspace", 43: "Tab", 44: "Space", 224: "Left Ctrl", 225: "Left Shift", 226: "Left Alt", 227: "Left Win", 228: "Right Ctrl", 229: "Right Shift", 230: "Right Alt", 231: "Right Win" })[number] || `HID 0x${number.toString(16).padStart(2, "0").toUpperCase()}`;
}

function macroEventValue(event) {
  if (event.type === "delay") return `${event.milliseconds} ms`;
  if (event.type.startsWith("key-")) return macroUsageLabel(event.usage);
  return macroMouseLabel(event.button);
}

function macroDuration(events = macroDraft?.events || []) {
  return events.reduce((total, event) => total + (event.type === "delay" ? Number(event.milliseconds || 0) : 0), 0);
}

function macroRenderLibrary() {
  const select = macroQ("#macroLibrary");
  const selected = macroDraft?.id;
  select.innerHTML = "";
  for (const program of macroPrograms) {
    const option = document.createElement("option");
    option.value = program.id;
    option.textContent = `${program.name} · ${program.events.length} events`;
    option.selected = program.id === selected;
    select.appendChild(option);
  }
}

function macroSetDraft(program) {
  macroStopRecording();
  macroDraft = macroClone(program);
  macroSelectedEvents.clear();
  macroQ("#macroName").value = macroDraft.name;
  macroQ("#macroPlayback").value = "once";
  macroQ("#macroSimulation").textContent = "No simulation run.";
  macroRenderLibrary();
  macroRenderTimeline();
  macroSetStatus(`${macroDraft.name} loaded. Hardware was not contacted.`);
}

function macroRenderTimeline() {
  const body = macroQ("#macroTimeline");
  body.innerHTML = "";
  if (!macroDraft) return;

  let elapsed = 0;
  macroDraft.events.forEach((event, index) => {
    const row = document.createElement("tr");
    row.classList.toggle("selected", macroSelectedEvents.has(index));

    const selectCell = document.createElement("td");
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.checked = macroSelectedEvents.has(index);
    checkbox.setAttribute("aria-label", `Select event ${index + 1}`);
    checkbox.addEventListener("change", () => {
      if (checkbox.checked) macroSelectedEvents.add(index);
      else macroSelectedEvents.delete(index);
      macroRenderTimeline();
    });
    selectCell.appendChild(checkbox);

    const timeCell = document.createElement("td");
    timeCell.textContent = event.type === "delay"
      ? `${elapsed}→${elapsed + Number(event.milliseconds || 0)} ms`
      : `${elapsed} ms`;

    const nameCell = document.createElement("td");
    nameCell.textContent = macroEventName(event);

    const valueCell = document.createElement("td");
    if (event.type === "delay") {
      const input = document.createElement("input");
      input.type = "number";
      input.min = "0";
      input.max = "3600000";
      input.value = String(event.milliseconds);
      input.setAttribute("aria-label", `Delay for event ${index + 1}`);
      input.addEventListener("change", () => {
        event.milliseconds = Math.max(0, Math.min(3600000, Number(input.value) || 0));
        macroMarkDirty();
        macroRenderTimeline();
      });
      valueCell.appendChild(input);
    } else if (event.type.startsWith("key-")) {
      const input = document.createElement("input");
      input.type = "number";
      input.min = "1";
      input.max = "65535";
      input.value = String(event.usage);
      input.title = macroEventValue(event);
      input.setAttribute("aria-label", `HID usage for event ${index + 1}`);
      input.addEventListener("change", () => {
        event.usage = Math.max(1, Math.min(65535, Number(input.value) || 1));
        macroMarkDirty();
        macroRenderTimeline();
      });
      valueCell.appendChild(input);
    } else {
      const select = document.createElement("select");
      for (const [value, label] of [["left", "Mouse 1"], ["right", "Mouse 2"], ["middle", "Mouse 3"], ["button4", "Mouse 4"], ["button5", "Mouse 5"]]) {
        const option = document.createElement("option");
        option.value = value;
        option.textContent = label;
        option.selected = value === event.button;
        select.appendChild(option);
      }
      select.addEventListener("change", () => {
        event.button = select.value;
        macroMarkDirty();
      });
      valueCell.appendChild(select);
    }

    row.append(selectCell, timeCell, nameCell, valueCell);
    body.appendChild(row);
    if (event.type === "delay") elapsed += Number(event.milliseconds || 0);
  });

  macroQ("#macroEventCount").textContent = `${macroDraft.events.length} event${macroDraft.events.length === 1 ? "" : "s"}`;
  macroQ("#macroDuration").textContent = `${elapsed} ms`;
}

function macroDefaultEvent(type) {
  switch (type) {
    case "key-down": return { type, usage: 4 };
    case "key-up": return { type, usage: 4 };
    case "mouse-down": return { type, button: "left" };
    case "mouse-up": return { type, button: "left" };
    default: return { type: "delay", milliseconds: 100 };
  }
}

function macroInsertEvent() {
  if (!macroDraft) return;
  const event = macroDefaultEvent(macroQ("#insertMacroEvent").value);
  const selected = [...macroSelectedEvents].sort((a, b) => a - b);
  const position = selected.length ? selected[selected.length - 1] + 1 : macroDraft.events.length;
  macroDraft.events.splice(position, 0, event);
  macroSelectedEvents = new Set([position]);
  macroMarkDirty();
  macroRenderTimeline();
}

function macroDeleteEvents() {
  if (!macroDraft || macroSelectedEvents.size === 0) return;
  macroDraft.events = macroDraft.events.filter((_, index) => !macroSelectedEvents.has(index));
  macroSelectedEvents.clear();
  macroMarkDirty();
  macroRenderTimeline();
}

function macroDuplicateEvents() {
  if (!macroDraft || macroSelectedEvents.size === 0) return;
  const selected = [...macroSelectedEvents].sort((a, b) => a - b);
  const copies = selected.map((index) => macroClone(macroDraft.events[index]));
  const position = selected[selected.length - 1] + 1;
  macroDraft.events.splice(position, 0, ...copies);
  macroSelectedEvents = new Set(copies.map((_, index) => position + index));
  macroMarkDirty();
  macroRenderTimeline();
}

function macroMoveEvent(direction) {
  if (!macroDraft || macroSelectedEvents.size !== 1) return;
  const index = [...macroSelectedEvents][0];
  const target = index + direction;
  if (target < 0 || target >= macroDraft.events.length) return;
  [macroDraft.events[index], macroDraft.events[target]] = [macroDraft.events[target], macroDraft.events[index]];
  macroSelectedEvents = new Set([target]);
  macroMarkDirty();
  macroRenderTimeline();
}

function macroCopyEvents() {
  if (!macroDraft) return;
  macroEventClipboard = [...macroSelectedEvents]
    .sort((a, b) => a - b)
    .map((index) => macroClone(macroDraft.events[index]));
  macroSetStatus(`${macroEventClipboard.length} event${macroEventClipboard.length === 1 ? "" : "s"} copied.`);
}

function macroPasteEvents() {
  if (!macroDraft || macroEventClipboard.length === 0) return;
  const selected = [...macroSelectedEvents].sort((a, b) => a - b);
  const position = selected.length ? selected[selected.length - 1] + 1 : macroDraft.events.length;
  const copies = macroClone(macroEventClipboard);
  macroDraft.events.splice(position, 0, ...copies);
  macroSelectedEvents = new Set(copies.map((_, index) => position + index));
  macroMarkDirty();
  macroRenderTimeline();
}

function macroUsageFromCode(code) {
  if (/^Key[A-Z]$/.test(code)) return 4 + code.charCodeAt(3) - 65;
  const digits = { Digit1: 30, Digit2: 31, Digit3: 32, Digit4: 33, Digit5: 34, Digit6: 35, Digit7: 36, Digit8: 37, Digit9: 38, Digit0: 39 };
  const fixed = {
    ...digits,
    Enter: 40, Escape: 41, Backspace: 42, Tab: 43, Space: 44,
    Minus: 45, Equal: 46, BracketLeft: 47, BracketRight: 48, Backslash: 49,
    Semicolon: 51, Quote: 52, Backquote: 53, Comma: 54, Period: 55, Slash: 56,
    CapsLock: 57, PrintScreen: 70, ScrollLock: 71, Pause: 72, Insert: 73,
    Home: 74, PageUp: 75, Delete: 76, End: 77, PageDown: 78,
    ArrowRight: 79, ArrowLeft: 80, ArrowDown: 81, ArrowUp: 82,
    ControlLeft: 224, ShiftLeft: 225, AltLeft: 226, MetaLeft: 227,
    ControlRight: 228, ShiftRight: 229, AltRight: 230, MetaRight: 231
  };
  if (fixed[code]) return fixed[code];
  const functionMatch = /^F([1-9]|1[0-2])$/.exec(code);
  return functionMatch ? 57 + Number(functionMatch[1]) : null;
}

function macroMouseButton(button) {
  return ["left", "middle", "right", "button4", "button5"][button] || null;
}

function macroRecordEvent(event) {
  if (!macroDraft) return;
  const now = performance.now();
  const delay = Math.max(0, Math.round(now - macroRecordLast));
  if (delay > 0) macroDraft.events.push({ type: "delay", milliseconds: Math.min(delay, 3600000) });
  macroDraft.events.push(event);
  macroRecordLast = now;
  macroMarkDirty("Recording macro events...");
  macroRenderTimeline();
}

function macroStartRecording() {
  if (!macroDraft || macroRecording) return;
  macroRecording = true;
  macroRecordLast = performance.now();
  macroQ("#recordMacro").disabled = true;
  macroQ("#stopMacro").disabled = false;
  macroQ("#macroCapturePad").classList.add("recording");
  macroQ("#macroCaptureTitle").textContent = "Recording";
  macroQ("#macroCaptureHelp").textContent = "Keys and mouse buttons used inside this area are being recorded.";
  macroQ("#macroCapturePad").focus();
  macroSetStatus("Recording. Hardware is not involved.");
}

function macroStopRecording() {
  if (!macroRecording) return;
  macroRecording = false;
  macroQ("#recordMacro").disabled = false;
  macroQ("#stopMacro").disabled = true;
  macroQ("#macroCapturePad").classList.remove("recording");
  macroQ("#macroCaptureTitle").textContent = "Recording area";
  macroQ("#macroCaptureHelp").textContent = "Choose Record, then press keys or mouse buttons here. Choose Stop when finished.";
  macroSetStatus("Recording stopped. Save when the timeline is ready.");
}

async function macroSave() {
  if (!macroDraft) return;
  macroDraft.name = macroQ("#macroName").value.trim();
  macroDraft.playback = { mode: "once" };
  try {
    const saved = await macroInvoke("save_macro", { program: macroDraft });
    const index = macroPrograms.findIndex((program) => program.id === saved.id);
    if (index >= 0) macroPrograms[index] = saved;
    else macroPrograms.push(saved);
    macroPrograms.sort((a, b) => a.name.localeCompare(b.name));
    macroSetDraft(saved);
    macroSetStatus(`${saved.name} saved locally. No keyboard command was sent.`);
  } catch (error) {
    macroSetStatus(String(error), true);
  }
}

async function macroDelete() {
  if (!macroDraft) return;
  try {
    await macroInvoke("delete_macro", { macroId: macroDraft.id });
    macroPrograms = macroPrograms.filter((program) => program.id !== macroDraft.id);
    const next = macroPrograms[0] || newMacroProgram();
    macroSetDraft(next);
    macroSetStatus("Macro deleted from local storage.");
  } catch (error) {
    macroSetStatus(String(error), true);
  }
}

function macroDuplicateProgram() {
  if (!macroDraft) return;
  const copy = macroClone(macroDraft);
  copy.id = macroId();
  copy.name = `${copy.name} copy`;
  macroSetDraft(copy);
  macroMarkDirty("Macro duplicated. Save to keep the copy.");
}

async function macroSimulate() {
  if (!macroDraft) return;
  macroDraft.name = macroQ("#macroName").value.trim();
  try {
    const result = await macroInvoke("simulate_macro", { program: macroDraft });
    const lines = result.steps.map((step) => `${String(step.elapsedMilliseconds).padStart(6)} ms  ${macroEventName(step.event)}  ${macroEventValue(step.event)}`);
    macroQ("#macroSimulation").textContent = [
      `Duration: ${result.durationMilliseconds} ms`,
      `Hardware touched: ${result.hardwareTouched ? "YES" : "NO"}`,
      "",
      ...(lines.length ? lines : ["No key or mouse events."])
    ].join("\n");
    macroSetStatus("Simulation complete. The keyboard was not contacted.");
  } catch (error) {
    macroSetStatus(String(error), true);
  }
}

async function macroExport() {
  if (!macroDraft) return;
  macroDraft.name = macroQ("#macroName").value.trim();
  const json = JSON.stringify(macroDraft, null, 2);
  macroQ("#macroJson").value = json;
  try { await navigator.clipboard.writeText(json); } catch (_) { /* textarea remains available */ }
  macroSetStatus("Macro exported as JSON.");
}

async function macroImport() {
  try {
    const program = JSON.parse(macroQ("#macroJson").value);
    await macroInvoke("simulate_macro", { program });
    macroSetDraft(program);
    macroMarkDirty("Imported macro validated. Save to keep it.");
  } catch (error) {
    macroSetStatus(`Import failed: ${error}`, true);
  }
}

async function macroLoadLibrary() {
  try {
    macroPrograms = await macroInvoke("get_macros");
    macroSetDraft(macroPrograms[0] || newMacroProgram());
    macroSetStatus(macroPrograms.length ? `${macroPrograms.length} macro${macroPrograms.length === 1 ? "" : "s"} loaded.` : "No saved macros. Create the first one.");
  } catch (error) {
    macroSetDraft(newMacroProgram());
    macroSetStatus(String(error), true);
  }
}

function macroBind() {
  macroQ("#macroLibrary").addEventListener("change", () => {
    const program = macroPrograms.find((item) => item.id === macroQ("#macroLibrary").value);
    if (program) macroSetDraft(program);
  });
  macroQ("#newMacro").addEventListener("click", () => macroSetDraft(newMacroProgram()));
  macroQ("#duplicateMacro").addEventListener("click", macroDuplicateProgram);
  macroQ("#deleteMacro").addEventListener("click", macroDelete);
  macroQ("#saveMacro").addEventListener("click", macroSave);
  macroQ("#macroName").addEventListener("input", () => {
    if (macroDraft) macroDraft.name = macroQ("#macroName").value;
    macroMarkDirty();
  });
  macroQ("#insertMacroEventButton").addEventListener("click", macroInsertEvent);
  macroQ("#deleteMacroEvents").addEventListener("click", macroDeleteEvents);
  macroQ("#duplicateMacroEvent").addEventListener("click", macroDuplicateEvents);
  macroQ("#moveMacroEventUp").addEventListener("click", () => macroMoveEvent(-1));
  macroQ("#moveMacroEventDown").addEventListener("click", () => macroMoveEvent(1));
  macroQ("#copyMacroEvents").addEventListener("click", macroCopyEvents);
  macroQ("#pasteMacroEvents").addEventListener("click", macroPasteEvents);
  macroQ("#recordMacro").addEventListener("click", macroStartRecording);
  macroQ("#stopMacro").addEventListener("click", macroStopRecording);
  macroQ("#simulateMacro").addEventListener("click", macroSimulate);
  macroQ("#exportMacro").addEventListener("click", macroExport);
  macroQ("#importMacro").addEventListener("click", macroImport);

  const pad = macroQ("#macroCapturePad");
  pad.addEventListener("keydown", (event) => {
    if (!macroRecording || event.repeat) return;
    const usage = macroUsageFromCode(event.code);
    if (usage == null) return;
    event.preventDefault();
    macroRecordEvent({ type: "key-down", usage });
  });
  pad.addEventListener("keyup", (event) => {
    if (!macroRecording) return;
    const usage = macroUsageFromCode(event.code);
    if (usage == null) return;
    event.preventDefault();
    macroRecordEvent({ type: "key-up", usage });
  });
  pad.addEventListener("mousedown", (event) => {
    if (!macroRecording) return;
    const button = macroMouseButton(event.button);
    if (!button) return;
    event.preventDefault();
    pad.focus();
    macroRecordEvent({ type: "mouse-down", button });
  });
  pad.addEventListener("mouseup", (event) => {
    if (!macroRecording) return;
    const button = macroMouseButton(event.button);
    if (!button) return;
    event.preventDefault();
    macroRecordEvent({ type: "mouse-up", button });
  });
  pad.addEventListener("contextmenu", (event) => event.preventDefault());
}

macroBind();
void macroLoadLibrary();
