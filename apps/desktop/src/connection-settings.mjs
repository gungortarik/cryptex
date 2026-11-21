const states = new Set(["maintenance", "maintenance_attention", "not_enrolled", "paused", "address_needed", "applying", "ready_locally", "attention"]);
const errors = new Set(["invalid_request", "unauthorized", "unavailable", "not_enrolled", "stale_revision", "unknown_device", "invalid_endpoint", "storage", "pending_settings", "authority", "local_restricted", "shutdown", "start", "start_rest_deadline", "held"]);
function exact(value, keys) {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
}
export function privateIPv4(value) {
  if (typeof value !== "string") return false;
  const parts = value.split(".");
  if (parts.length !== 4 || parts.some(p => !/^(0|[1-9]\d{0,2})$/.test(p) || Number(p) > 255)) return false;
  const [a, b] = parts.map(Number);
  return a === 10 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168);
}
export function publicEndpoint(value) {
  if (typeof value !== "string") return false;
  const [ip, port, extra] = value.split(":");
  return extra === undefined && privateIPv4(ip) && /^[1-9]\d{0,4}$/.test(port ?? "") && Number(port) <= 65535;
}
export function parseConnectionView(value) {
  if (!exact(value, ["revision", "paused", "listenAddress", "localTransportPort", "devices", "state", "error"])
    || typeof value.revision !== "string" || !/^(0|[1-9]\d{0,19})$/.test(value.revision)
    || BigInt(value.revision) > 18446744073709551615n
    || typeof value.paused !== "boolean" || !states.has(value.state)
    || (value.listenAddress !== null && !privateIPv4(value.listenAddress))
    || (value.localTransportPort !== null && (!Number.isInteger(value.localTransportPort) || value.localTransportPort < 1 || value.localTransportPort > 65535))
    || (value.error !== null && !errors.has(value.error))
    || !Array.isArray(value.devices) || value.devices.length > 64) throw new Error("connection_view_invalid");
  const ids = new Set();
  for (const device of value.devices) {
    if (!exact(device, ["deviceId", "transportId", "endpoint"])
      || typeof device.deviceId !== "string" || (!/^[A-Za-z0-9_-]{1,64}$/.test(device.deviceId) || /^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])$/i.test(device.deviceId))
      || typeof device.transportId !== "string" || !/^[A-Z2-7]{7}(?:-[A-Z2-7]{7}){7}$/.test(device.transportId)
      || (device.endpoint !== null && !publicEndpoint(device.endpoint)) || ids.has(device.deviceId)) throw new Error("connection_view_invalid");
    ids.add(device.deviceId);
  }
  return value;
}
export function connectionUpdate(view, paused, listen, addresses) {
  parseConnectionView(view);
  const ip = listen.trim();
  if (ip && !privateIPv4(ip)) throw new Error("Use a private LAN IPv4 address, such as 192.168.1.20.");
  const seen = new Set(ip && view.localTransportPort ? [`${ip}:${view.localTransportPort}`] : []);
  const peers = [];
  for (const device of view.devices) {
    const endpoint = (addresses[device.deviceId] ?? "").trim();
    if (!endpoint) continue;
    if (!publicEndpoint(endpoint)) throw new Error("Use a private LAN address and transport port, such as 192.168.1.21:22000.");
    if (seen.has(endpoint)) throw new Error("Each device needs a different LAN endpoint.");
    seen.add(endpoint);
    peers.push({ deviceId: device.deviceId, endpoint });
  }
  return { expectedRevision: view.revision, paused, listenAddress: ip || null, peers };
}
export function connectionMessage(view) {
  if (view.state === "maintenance") return "Maintenance Mode is on. Sync and AI access are paused. You can still read and save notes.";
  if (view.state === "maintenance_attention") return "AI access is paused. The service could not confirm that sync stopped.";
  if (view.state === "attention" && view.error === "start_rest_deadline") return "Connection needs attention (start_rest_deadline). The local transport service timed out during startup.";
  if (view.error === "local_restricted") return "This device is retired or restricted. Sync is unavailable; retained local history remains subject to its verified access policy.";
  if (view.error && view.state !== "attention" && !(view.state === "not_enrolled" && view.error === "not_enrolled"))
    return "Connection status could not be verified.";
  if (view.state === "attention") {
    const code = errors.has(view.error) ? ` (${view.error})` : "";
    return `Connection needs attention${code}. Local settings or device authority could not be verified.`;
  }
  return ({ not_enrolled: "This device has no enrolled connection profile yet.", paused: "Sync paused. Your local memory remains available.",
    address_needed: "Add a LAN address for this device and each trusted device to connect.", applying: "Applying connection settings…",
    ready_locally: "Connection ready on this device. Remote durable storage has not been verified.",
    })[view.state];
}
