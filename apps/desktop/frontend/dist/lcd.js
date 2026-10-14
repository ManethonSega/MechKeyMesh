(() => {
  const invoke = (command, args = {}) => window.__TAURI__.core.invoke(command, args);
  const element = (selector) => document.querySelector(selector);
  const canvas = element("#lcdCanvas");
  const context = canvas.getContext("2d", { alpha: false });
  let editorState = null;
  let selectedFrame = 0;
  let drawing = false;
  let points = [];

  function setStatus(message, error = false) {
    const status = element("#lcdStatus");
    status.textContent = String(message);
    status.style.color = error ? "#ff806b" : "";
  }

  function colorRgba() {
    const value = parseInt(element("#lcdColor").value.slice(1), 16);
    return [(value >> 16) & 255, (value >> 8) & 255, value & 255, 255];
  }

  async function loadPreview(index = selectedFrame) {
    if (!editorState) return;
    const source = await invoke("get_lcd_frame_preview", { index });
    const image = new Image();
    await new Promise((resolve, reject) => {
      image.onload = resolve;
      image.onerror = reject;
      image.src = source;
    });
    context.drawImage(image, 0, 0, 240, 135);
    return source;
  }

  async function renderState(state, rebuildTimeline = true) {
    editorState = state;
    selectedFrame = Math.min(selectedFrame, state.project.frameCount - 1);
    element("#lcdValidity").textContent = state.project.valid ? "Valid F108 project" : "Invalid";
    element("#lcdValidity").className = "verification " + (state.project.valid ? "verified" : "pending");
    element("#lcdProjectTitle").textContent = state.project.name;
    element("#lcdProjectSummary").textContent = `${state.project.frameCount} / 141 frames · ${(state.project.totalDurationMs / 1000).toFixed(2)} s`;
    element("#lcdFrameTitle").textContent = `Frame ${selectedFrame + 1}`;
    element("#lcdDuration").value = state.durationsMs[selectedFrame];
    element("#lcdUndo").disabled = !state.canUndo;
    element("#lcdRedo").disabled = !state.canRedo;
    await loadPreview();
    if (!rebuildTimeline) return;

    const timeline = element("#lcdTimeline");
    timeline.replaceChildren();
    for (let index = 0; index < state.project.frameCount; index += 1) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "lcd-frame-thumb" + (index === selectedFrame ? " active" : "");
      button.innerHTML = `<img alt="Frame ${index + 1}"/><span>${index + 1} · ${state.durationsMs[index]} ms</span>`;
      button.addEventListener("click", async () => {
        selectedFrame = index;
        document.querySelectorAll(".lcd-frame-thumb").forEach((item, itemIndex) => item.classList.toggle("active", itemIndex === index));
        element("#lcdFrameTitle").textContent = `Frame ${index + 1}`;
        element("#lcdDuration").value = editorState.durationsMs[index];
        await loadPreview();
      });
      timeline.append(button);
      invoke("get_lcd_frame_preview", { index })
        .then((source) => { button.querySelector("img").src = source; })
        .catch(() => {});
    }
  }

  async function run(command, args = {}) {
    try {
      const state = await invoke(command, args);
      await renderState(state);
      setStatus("Project validated locally. No keyboard data was sent.");
    } catch (error) {
      setStatus(error, true);
    }
  }

  element("#lcdImport").addEventListener("click", async () => {
    try {
      setStatus("Opening the media browser...");
      const path = await invoke("select_lcd_media_file");
      if (!path) {
        setStatus("No media file selected.");
        return;
      }
      element("#lcdMediaPath").value = path;
      setStatus("Decoding and optimizing media...");
      selectedFrame = 0;
      await run("import_lcd_media", {
        path,
        preset: element("#lcdPreset").value,
        fit: element("#lcdFit").value
      });
    } catch (error) {
      setStatus(error, true);
    }
  });

  element("#lcdDurationApply").addEventListener("click", () => run("set_lcd_frame_duration", {
    index: selectedFrame,
    durationMs: Number(element("#lcdDuration").value)
  }));
  element("#lcdDuplicate").addEventListener("click", () => run("duplicate_lcd_frame", { index: selectedFrame }));
  element("#lcdDelete").addEventListener("click", () => run("delete_lcd_frame", { index: selectedFrame }));
  element("#lcdMoveLeft").addEventListener("click", () => {
    if (selectedFrame === 0) return;
    const from = selectedFrame;
    selectedFrame -= 1;
    void run("reorder_lcd_frame", { from, to: selectedFrame });
  });
  element("#lcdMoveRight").addEventListener("click", () => {
    if (!editorState || selectedFrame + 1 >= editorState.project.frameCount) return;
    const from = selectedFrame;
    selectedFrame += 1;
    void run("reorder_lcd_frame", { from, to: selectedFrame });
  });
  element("#lcdUndo").addEventListener("click", () => run("undo_lcd_edit"));
  element("#lcdRedo").addEventListener("click", () => run("redo_lcd_edit"));
  element("#lcdAddText").addEventListener("click", () => run("add_lcd_text", {
    index: selectedFrame,
    x: 8,
    y: 8,
    text: element("#lcdText").value,
    scale: Number(element("#lcdTextScale").value),
    color: colorRgba()
  }));

  function canvasPoint(event) {
    const rect = canvas.getBoundingClientRect();
    return {
      x: Math.max(0, Math.min(239, Math.round((event.clientX - rect.left) * 240 / rect.width))),
      y: Math.max(0, Math.min(134, Math.round((event.clientY - rect.top) * 135 / rect.height)))
    };
  }

  canvas.addEventListener("pointerdown", (event) => {
    if (!editorState || element("#lcdTool").value === "select") return;
    drawing = true;
    points = [canvasPoint(event)];
    canvas.setPointerCapture(event.pointerId);
  });
  canvas.addEventListener("pointermove", (event) => {
    if (drawing) points.push(canvasPoint(event));
  });
  canvas.addEventListener("pointerup", async () => {
    if (!drawing) return;
    drawing = false;
    const tool = element("#lcdTool").value;
    const command = tool === "eraser" ? "erase_lcd" : "draw_lcd_brush";
    const args = {
      index: selectedFrame,
      points,
      radius: Number(element("#lcdBrushSize").value)
    };
    if (command === "draw_lcd_brush") args.color = colorRgba();
    await run(command, args);
  });

  element("#lcdExport").addEventListener("click", async () => {
    try {
      const summary = await invoke("export_lcd_asset", { path: element("#lcdExportPath").value.trim() });
      setStatus(`Exported ${summary.payloadLength} bytes in ${summary.pageCount} pages. Still no keyboard write.`);
    } catch (error) {
      setStatus(error, true);
    }
  });

  element("#lcdUpload").addEventListener("click", async () => {
    element("#lcdUpload").disabled = true;
    element("#lcdCancelUpload").disabled = false;
    setStatus("Uploading over the dedicated LCD interface. Do not disconnect the keyboard.");
    try {
      await invoke("upload_lcd_asset");
      setStatus("LCD upload completed and applied.");
    } catch (error) {
      setStatus(`LCD upload did not complete: ${error}`, true);
    } finally {
      element("#lcdUpload").disabled = false;
      element("#lcdCancelUpload").disabled = true;
      await pollTransfer();
    }
  });

  element("#lcdCancelUpload").addEventListener("click", async () => {
    const cancelled = await invoke("cancel_active_operation");
    if (cancelled) setStatus("Cancellation requested. The current page will stop safely.");
  });

  element("#lcdSyncClock").addEventListener("click", async () => {
    const now = new Date();
    try {
      await invoke("sync_lcd_clock", { clock: {
        year: now.getFullYear(),
        month: now.getMonth() + 1,
        day: now.getDate(),
        hour: now.getHours(),
        minute: now.getMinutes(),
        second: now.getSeconds(),
        weekdaySundayZero: now.getDay()
      }});
      setStatus("Keyboard clock synchronized.");
    } catch (error) {
      setStatus(error, true);
    }
  });

  async function pollTransfer() {
    try {
      const transfer = await invoke("get_lcd_transfer_status");
      const progress = element("#lcdUploadProgress");
      progress.max = Math.max(1, transfer.totalPages);
      progress.value = transfer.pagesSent;
      element("#lcdCancelUpload").disabled = !transfer.active;
      if (transfer.active) {
        element("#lcdUploadStatus").textContent = `Page ${transfer.pagesSent} / ${transfer.totalPages}`;
      } else if (transfer.incomplete) {
        element("#lcdUploadStatus").textContent = `Incomplete transfer at page ${transfer.pagesSent} / ${transfer.totalPages}. Reconnect and upload again.`;
      } else if (transfer.totalPages > 0) {
        element("#lcdUploadStatus").textContent = `Complete: ${transfer.totalPages} pages acknowledged.`;
      }
    } catch (_) {}
  }

  setInterval(pollTransfer, 250);
  void pollTransfer();

  invoke("get_lcd_editor_state").then((state) => state && renderState(state)).catch(() => {});
})();
