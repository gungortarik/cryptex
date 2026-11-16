import test from "node:test";
import assert from "node:assert/strict";
import { privateIPv4, publicEndpoint, parseConnectionView, connectionUpdate, connectionMessage } from "./connection-settings.mjs";
const id = "11111111-1111-4111-8111-111111111111";
const transport = "U2SYIGU-YWZX757-2WPZGCR-DHD7MLH-ZKV7HLJ-EYO6CXE-K3DU3DU-SDFXYAE";
const view = () => ({ revision: "4", paused: true, listenAddress: null, localTransportPort: 22000, devices: [{ deviceId: id, transportId: transport, endpoint: null }], state: "paused", error: null });
test("only explicit private IPv4 LAN endpoints are accepted", () => {
  for (const ip of ["10.0.0.1","172.16.1.1","172.31.255.1","192.168.0.1"]) assert.equal(privateIPv4(ip),true);
  for (const ip of ["127.0.0.1","0.0.0.0","172.32.0.1","8.8.8.8","169.254.1.1","::1","localhost","192.168.01.1"]) assert.equal(privateIPv4(ip),false);
  for (const ep of ["192.168.0.1:0","192.168.0.1:65536","tcp://192.168.0.1:22000","host:22000","192.168.0.1:22000/path","192.168.0.1:022000"]) assert.equal(publicEndpoint(ep),false);
  assert.equal(publicEndpoint("192.168.0.1:65535"),true);
});
test("reply shape refuses paths, process controls, duplicate peers and incompatible versions", () => {
  assert.deepEqual(parseConnectionView(view()),view());
  for (const patch of [{path:"/tmp"},{apiPort:8384},{pid:42},{revision:"04"},{revision:"18446744073709551616"},{state:"replicated"},{devices:[...view().devices,...view().devices]}]) assert.throws(() => parseConnectionView({...view(),...patch}));
});
test("update carries current revision and only canonical listed devices", () => {
  assert.deepEqual(connectionUpdate(view(), false, " 192.168.1.2 ",{[id]:"192.168.1.3:22000",unknown:"8.8.8.8:1"}),{expectedRevision:"4",paused:false,listenAddress:"192.168.1.2",peers:[{deviceId:id,endpoint:"192.168.1.3:22000"}]});
});
test("missing endpoint remains incomplete instead of inventing discovery", () => {
  assert.deepEqual(connectionUpdate(view(),false,"",{}).peers,[]);
  assert.equal(connectionUpdate(view(),true,"",{}).listenAddress,null);
});
test("same LAN endpoint cannot describe local and remote devices", () => {
  assert.throws(() => connectionUpdate(view(),false,"192.168.1.2",{[id]:"192.168.1.2:22000"}));
});
test("local readiness never claims remote durable storage", () => {
  assert.match(connectionMessage({...view(),state:"ready_locally"}),/Remote durable storage has not been verified/);
});

test("attention shows only allowlisted diagnostic codes, never raw service output", () => {
  for (const error of ["held", "authority", "start", "shutdown", "storage", "pending_settings"]) {
    assert.match(connectionMessage({...view(),state:"attention",error}),new RegExp(`\\(${error}\\)`));
  }
  for (const error of [null,"secret raw error C:/private",{message:"secret"}]) {
    assert.equal(connectionMessage({...view(),state:"attention",error}),
      "Connection needs attention. Local settings or device authority could not be verified.");
  }
});

test("opaque case-sensitive canonical IDs are preserved", () => {
  const v = view();
  v.devices = ["Device-A","device-a"].map(deviceId => ({deviceId,transportId:transport,endpoint:null}));
  assert.deepEqual(parseConnectionView(v).devices.map(d=>d.deviceId),["Device-A","device-a"]);
  for (const deviceId of ["CON","COM1","LPT9","../x","a".repeat(65)]) assert.throws(() => parseConnectionView({...v,devices:[{...v.devices[0],deviceId}]}));
});
