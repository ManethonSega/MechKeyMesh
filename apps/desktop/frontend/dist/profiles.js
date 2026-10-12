(() => {
  const invoke = (command, args = {}) => window.__TAURI__.core.invoke(command, args);
  const $ = (selector) => document.querySelector(selector);
  let profiles = [];
  let rules = [];

  function status(message, error = false) {
    $("#profileStatus").textContent = String(message);
    $("#profileStatus").style.color = error ? "#ff806b" : "";
  }

  async function refreshProfiles() {
    profiles = await invoke("get_profiles");
    const current = $("#profileList").value;
    $("#profileList").replaceChildren(...profiles.map((profile) =>
      new Option(`${profile.name}${profile.isDefault ? " · default" : ""}`, profile.id)
    ));
    $("#ruleProfile").replaceChildren(...profiles.map((profile) =>
      new Option(profile.name, profile.id)
    ));
    if (profiles.some((profile) => profile.id === current)) $("#profileList").value = current;
    await refreshRules();
  }

  async function refreshRules() {
    rules = await invoke("get_application_rules");
    const list = $("#ruleList");
    list.replaceChildren();
    for (const rule of rules) {
      const row = document.createElement("div");
      row.className = "rule-row";
      const profile = profiles.find((item) => item.id === rule.profileId);
      const pattern = document.createElement("strong");
      pattern.textContent = rule.executablePattern;
      const profileName = document.createElement("span");
      profileName.textContent = profile?.name || rule.profileId;
      const priority = document.createElement("span");
      priority.textContent = `P${rule.priority}`;
      const remove = document.createElement("button");
      remove.className = "secondary compact danger-soft";
      remove.type = "button";
      remove.textContent = "Delete";
      row.replaceChildren(pattern, profileName, priority, remove);
      row.querySelector("button").addEventListener("click", async () => {
        await invoke("delete_application_rule", { profileId: rule.profileId, id: rule.id });
        await refreshRules();
      });
      list.append(row);
    }
  }

  $("#profileCreate").addEventListener("click", async () => {
    try {
      await invoke("create_profile", { id: $("#profileId").value.trim(), name: $("#profileName").value.trim() });
      await refreshProfiles();
      status("Profile cloned.");
    } catch (error) { status(error, true); }
  });
  $("#profileActivate").addEventListener("click", async () => {
    try {
      const report = await invoke("activate_profile", { profileId: $("#profileList").value });
      renderReport(report);
      await pollRuntime();
    } catch (error) { status(error, true); }
  });
  $("#profileRename").addEventListener("click", async () => {
    try {
      await invoke("rename_profile", { profileId: $("#profileList").value, name: $("#profileName").value.trim() });
      await refreshProfiles();
    } catch (error) { status(error, true); }
  });
  $("#profileDelete").addEventListener("click", async () => {
    try {
      await invoke("delete_profile", { profileId: $("#profileList").value });
      await refreshProfiles();
    } catch (error) { status(error, true); }
  });
  $("#profileAutoSwitch").addEventListener("change", () => invoke("set_profile_auto_switch", { enabled: $("#profileAutoSwitch").checked }));
  $("#ruleAdd").addEventListener("click", async () => {
    const pattern = $("#rulePattern").value.trim();
    const profileId = $("#ruleProfile").value;
    const id = `${profileId}-${pattern.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "")}`;
    try {
      await invoke("save_application_rule", { rule: {
        id,
        deviceModelId: "aula.f108pro",
        profileId,
        executablePattern: pattern,
        priority: Number($("#rulePriority").value),
        enabled: true
      }});
      await refreshRules();
    } catch (error) { status(error, true); }
  });
  $("#profileAssignLcd").addEventListener("click", async () => {
    try {
      await invoke("assign_lcd_asset_to_profile", { path: $("#profileLcdPath").value.trim() });
      status("Validated LCD asset assigned to active profile.");
    } catch (error) { status(error, true); }
  });

  function renderReport(report) {
    if (!report) return;
    $("#profileApplyReport").textContent = [
      `Profile: ${report.profileId}`,
      `Changed: ${report.changed.join(", ") || "none"}`,
      `Skipped: ${report.skipped.join(", ") || "none"}`,
      `Errors: ${report.errors.join(", ") || "none"}`
    ].join("\n");
  }

  async function pollRuntime() {
    try {
      const runtime = await invoke("get_profile_runtime_status");
      $("#activeProfileTag").textContent = profiles.find((profile) => profile.id === runtime.activeProfileId)?.name || runtime.activeProfileId;
      $("#desiredProfile").textContent = runtime.activeProfileId;
      $("#profileAutoSwitch").checked = runtime.autoSwitch;
      $("#profileForeground").textContent = runtime.foregroundExecutable || "Unavailable";
      renderReport(runtime.lastReport);
    } catch (_) {}
  }

  refreshProfiles().then(pollRuntime).catch((error) => status(error, true));
  setInterval(pollRuntime, 750);
})();
