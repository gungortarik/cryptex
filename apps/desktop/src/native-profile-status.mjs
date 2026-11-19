// Diagnostic validation only; this value never grants write/trust authority.
const states = new Set(["idle", "waiting_for_transport", "deferred", "published", "rejected", "held", "unavailable"]);
export function parseNativeProfileStatus(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)
    || Object.keys(value).sort().join(",") !== "incoming,protection,remoteDurabilityVerified,schemaVersion,transportObserved"
    || value.schemaVersion !== 1 || value.remoteDurabilityVerified !== false
    || typeof value.transportObserved !== "boolean" || !states.has(value.incoming)) return null;
  const attention = ["held", "rejected", "unavailable"].includes(value.incoming);
  if (value.protection !== (attention ? "attention" : "local_only")) return null;
  return Object.freeze({ ...value });
}
export function nativeProfileMessage(status) {
  if (!status) return "Device status is unavailable. A remote copy has not been verified.";
  if (status.incoming === "held") return "Incoming changes are paused for verification.";
  if (status.incoming === "rejected") return "This device rejected incoming data during verification.";
  if (status.incoming === "unavailable") return "Incoming status is unavailable. A remote copy has not been verified.";
  if (status.incoming === "deferred") return "Incoming processing is deferred on this device.";
  if (status.incoming === "published") return "Incoming data was verified on this device. Remote durable storage has not been verified.";
  if (!status.transportObserved) return "Local transport has not been observed. Remote storage is unverified.";
  return "Local memory is available. A remote durable copy has not been verified.";
}
