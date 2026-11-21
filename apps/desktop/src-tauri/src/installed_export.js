(async () => {
  const mode = "EXPECTED_MODE";
  const panel = () => document.querySelector('[aria-labelledby="manual-backup-heading"]');
  let clicked = false;
  let visible = false;
  try {
    const deadline = Date.now() + 60000;
    while (Date.now() < deadline) {
      const button = [...(panel()?.querySelectorAll("button") ?? [])].find(el =>
        el.textContent === (mode.startsWith("verify") ? "Verify manual backup" : "Export encrypted backup"));
      if (!clicked && button && !button.disabled) {
        clicked = true;
        button.click();
      }
      const status = panel()?.querySelector('[role="status"]')?.textContent;
      visible = mode === "verify" ? status === "Backup verified by restoring it in temporary storage." :
        mode === "verify-cancel" ? status === "Verification cancelled." :
        mode.startsWith("verify-") ? status?.startsWith("Verification failed.") === true :
        ["wrong-brain", "damaged"].includes(mode) ? status === "Cryptex could not verify the Recovery Kit certificate for this profile." :
        ["decline", "cancel"].includes(mode) ? status === "Export cancelled." :
        mode === "overlap" ? status === "Choose two separate folders outside the Cryptex profile." :
        /^Backup written as manual-[0-9a-f]{32}\.pgp\./.test(status ?? "");
      if (visible || status) break;
      await new Promise(resolve => setTimeout(resolve, 50));
    }
  } finally {
    await window.__TAURI_INTERNALS__.invoke("m11_search_observed", { visible });
  }
})();
