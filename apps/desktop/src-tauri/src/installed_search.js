// Fixed QA action inside the installed system WebView. Search uses the ordinary form.
(async () => {
  const until = Date.now() + 30000;
  async function waitFor(find) {
    while (Date.now() < until) {
      const value = find();
      if (value) return value;
      await new Promise(resolve => setTimeout(resolve, 50));
    }
    throw Error("expected search screen was not visible");
  }
  let visible = false;
  try {
    const input = await waitFor(() => document.querySelector("form[role=search] #memory-search"));
    const button = input.form.querySelector("button[type=submit]");
    if (button?.textContent.trim() !== "Search") throw Error("Search button unavailable");
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(input, "prefix OR recovered");
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await new Promise(resolve => setTimeout(resolve, 0));
    button.click();
    await waitFor(() => {
      const bodies = Array.from(document.querySelectorAll('[aria-label="Search results"] li p'))
        .filter(el => el.getClientRects().length > 0).map(el => el.textContent);
      return bodies.includes("synthetic original prefix") && bodies.includes("First recovered device note");
    });
    visible = true;
  } finally {
    await window.__TAURI_INTERNALS__.invoke("m11_search_observed", { visible });
  }
})();
