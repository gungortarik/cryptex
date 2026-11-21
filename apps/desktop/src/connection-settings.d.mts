export type ConnectionView = {
  revision: string; paused: boolean; listenAddress: string | null; localTransportPort: number | null;
  devices: { deviceId: string; transportId: string; endpoint: string | null }[];
  state: "maintenance" | "maintenance_attention" | "not_enrolled" | "paused" | "address_needed" | "applying" | "ready_locally" | "attention"; error: string | null;
};
export function privateIPv4(value: unknown): boolean;
export function publicEndpoint(value: unknown): boolean;
export function parseConnectionView(value: unknown): ConnectionView;
export function connectionUpdate(view: ConnectionView, paused: boolean, listen: string, addresses: Record<string,string>): {
  expectedRevision: string; paused: boolean; listenAddress: string | null; peers: { deviceId: string; endpoint: string }[];
};
export function connectionMessage(view: ConnectionView): string;
