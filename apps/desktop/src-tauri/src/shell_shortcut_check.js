// Drive the mounted WebView. Keep a modal open for the first keypress.
(() => {
  const timeline = document.querySelector('button[aria-label="Open timeline"]');
  if (!timeline) return false;
  timeline.click();
  const modal = document.createElement("dialog");
  document.body.appendChild(modal);
  modal.showModal();
  const key = () => new KeyboardEvent("keydown", {
    key: "k", bubbles: true, cancelable: true,
    [navigator.platform.toLowerCase().includes("mac") ? "metaKey" : "ctrlKey"]: true,
  });
  const ignored = window.dispatchEvent(key());
  modal.close();
  modal.remove();
  return ignored;
})()
