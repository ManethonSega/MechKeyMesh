/* Small shared interactions; hardware commands remain in their existing modules. */
window.mkmSetSaveState = (id, state, text) => {
  const element = document.getElementById(id);
  if (!element) return;
  element.dataset.state = state;
  element.textContent = text;
};
window.mkmConfirmDelete = (title, description) => new Promise((resolve) => {
  const dialog = document.getElementById("confirmDialog");
  if (dialog.open) { resolve(false); return; }
  document.getElementById("confirmTitle").textContent = title;
  document.getElementById("confirmDescription").textContent = description;
  const trigger = document.activeElement;
  dialog.returnValue = "cancel";
  dialog.addEventListener("close", () => {
    trigger?.focus();
    resolve(dialog.returnValue === "confirm");
  }, { once: true });
  dialog.showModal();
});
document.addEventListener("click", (event) => {
  const shortcut = event.target.closest("[data-open-page]");
  if (shortcut && !shortcut.disabled) showPage(shortcut.dataset.openPage);
});
document.querySelectorAll('[role="tablist"]').forEach((list) => {
  const tabs = [...list.querySelectorAll('[role="tab"]')];
  function sync() {
    tabs.forEach((tab) => { tab.tabIndex = tab.getAttribute("aria-selected") === "true" ? 0 : -1; });
  }
  list.addEventListener("click", sync);
  list.addEventListener("keydown", (event) => {
    const enabled = tabs.filter((tab) => !tab.disabled);
    const current = enabled.indexOf(document.activeElement);
    if (current < 0) return;
    let next;
    if (event.key === "ArrowRight") next = (current + 1) % enabled.length;
    if (event.key === "ArrowLeft") next = (current + enabled.length - 1) % enabled.length;
    if (event.key === "Home") next = 0;
    if (event.key === "End") next = enabled.length - 1;
    if (next === undefined) return;
    event.preventDefault();
    enabled[next].click();
    enabled[next].focus();
    sync();
  });
  sync();
});
