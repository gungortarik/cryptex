export type NativeProfileStatus = Readonly<{
  schemaVersion: 1;
  protection: "local_only" | "attention";
  incoming: "idle" | "waiting_for_transport" | "deferred" | "published" | "rejected" | "held" | "unavailable";
  transportObserved: boolean;
  remoteDurabilityVerified: false;
}>;
export function parseNativeProfileStatus(value: unknown): NativeProfileStatus | null;
export function nativeProfileMessage(status: NativeProfileStatus | null): string;
