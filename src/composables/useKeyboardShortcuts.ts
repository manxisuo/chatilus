function isEditableElement(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName.toLowerCase();
  if (tag === "input" || tag === "textarea" || tag === "select") return true;
  if (target.isContentEditable) return true;
  return Boolean(target.closest('input, textarea, select, [contenteditable="true"]'));
}

function isSelectableTextArea(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return Boolean(target.closest(".selectable-text"));
}

function selectElementText(element: HTMLElement) {
  const selection = window.getSelection();
  if (!selection) return;
  const range = document.createRange();
  range.selectNodeContents(element);
  selection.removeAllRanges();
  selection.addRange(range);
}

function handleSelectAll(event: KeyboardEvent) {
  const target = event.target;

  if (isEditableElement(target)) {
    return;
  }

  const el = target instanceof HTMLElement ? target : null;
  const messageEl = el?.closest(".message");
  if (messageEl) {
    const content = messageEl.querySelector(".selectable-text");
    if (content instanceof HTMLElement) {
      event.preventDefault();
      selectElementText(content);
    }
    return;
  }

  if (el?.closest(".selectable-text")) {
    event.preventDefault();
    return;
  }

  event.preventDefault();
}

function onKeyDown(event: KeyboardEvent) {
  const isSelectAll =
    (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a";
  if (!isSelectAll) return;
  handleSelectAll(event);
}

export function installKeyboardShortcuts(): () => void {
  window.addEventListener("keydown", onKeyDown);
  return () => window.removeEventListener("keydown", onKeyDown);
}

function onContextMenu(event: MouseEvent) {
  const target = event.target;
  if (isEditableElement(target) || isSelectableTextArea(target)) {
    return;
  }
  event.preventDefault();
}

export function installContextMenuGuard(): () => void {
  window.addEventListener("contextmenu", onContextMenu);
  return () => window.removeEventListener("contextmenu", onContextMenu);
}

export function installDesktopBehaviors(): () => void {
  const cleanups = [installKeyboardShortcuts(), installContextMenuGuard()];
  return () => cleanups.forEach((cleanup) => cleanup());
}
