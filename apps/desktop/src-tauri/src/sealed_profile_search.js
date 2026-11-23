// Only embedded in the disposable installed-profile trial.
(async () => {
  const deadline = Date.now() + 45000;
  const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
  let nextSearch = 0;
  while (Date.now() < deadline) {
    const results = document.querySelectorAll('[aria-label="Search results"] li p');
    if ([...results].some(el => el.getClientRects().length &&
        el.textContent === "Synthetic signed transfer probe. No personal data.")) {
      await window.__TAURI_INTERNALS__.invoke("m17_search_observed");
      return;
    }
    const input = document.querySelector("form[role=search] #memory-search");
    const button = input?.form.querySelector("button[type=submit]");
    if (button && !button.disabled && Date.now() >= nextSearch) {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(input, "synthetic");
      input.dispatchEvent(new Event("input", { bubbles: true }));
      await pause(0);
      button.click();
      nextSearch = Date.now() + 3000;
    }
    await pause(100);
  }
})();
