import { test } from 'node:test';
import assert from 'node:assert/strict';
import { isolateSyncthingXml, startSyncthingPilot, assertSyncthingVersion, SYNCTHING_STOP_BUDGET_MS } from './syncthing-pilot.mjs';
const xml = '<configuration><folder id="default" path="/home/Sync"><paused>false</paused></folder><gui><address>127.0.0.1:8384</address><apikey>synthetic-test-key</apikey></gui><options>'
  + ['listenAddress', 'globalAnnounceEnabled', 'localAnnounceEnabled', 'relaysEnabled', 'natEnabled', 'autoUpgradeIntervalH', 'urAccepted'].map(name => `<${name}>true</${name}>`).join('')
  + '</options><defaults><folder id=""><path></path></folder></defaults></configuration>';
test('managed XML disables external discovery/relay/NAT and default visible folder', () => {
  const result = isolateSyncthingXml(xml, 42001, 42002);
  assert.ok(result.includes('<address>127.0.0.1:42001</address>'));
  assert.ok(result.includes('<listenAddress>tcp://127.0.0.1:42002</listenAddress>'));
  for (const name of ['globalAnnounceEnabled', 'localAnnounceEnabled', 'relaysEnabled', 'natEnabled']) assert.ok(result.includes(`<${name}>false</${name}>`));
  assert.equal(result.includes('/home/Sync'), false); assert.ok(result.includes('<defaults>'));
  assert.ok(result.includes('<autoUpgradeIntervalH>0</autoUpgradeIntervalH>'));
});
test('managed XML refuses duplicate GUI/options and missing isolation settings', () => {
  for (const value of [xml.replace('</configuration>', '<gui></gui></configuration>'), xml.replace('</configuration>', '<options></options></configuration>'), xml.replace('<natEnabled>true</natEnabled>', '')]) assert.throws(() => isolateSyncthingXml(value, 42001, 42002), { code: 'INVALID_SYNCTHING_CONFIG' });
});
test('managed XML refuses shared/invalid listener ports', () => {
  for (const ports of [[42001, 42001], [80, 42002], [42001, 70000]]) assert.throws(() => isolateSyncthingXml(xml, ...ports), { code: 'INVALID_SYNCTHING_PORT' });
});
test('managed launcher refuses relative binary or home before process launch', async () => {
  await assert.rejects(startSyncthingPilot('relative', 'relative'), { code: 'INVALID_SYNCTHING_ARGS' });
});
test('managed pin rejects prerelease and different versions', () => {
  assertSyncthingVersion('syncthing v2.1.5 "Hafnium Hornet"');
  for (const value of ['syncthing v2.1.2 x', 'syncthing v2.1.50 x', 'syncthing v2.1.5-rc.1 x', 'v2.1.5']) assert.throws(() => assertSyncthingVersion(value), { code: 'SYNCTHING_PIN_MISMATCH' });
});
test('managed config refuses missing or repeated GUI listener', () => {
  for (const value of [xml.replace('<address>127.0.0.1:8384</address>', ''), xml.replace('</gui>', '<address>0.0.0.0:8384</address></gui>')]) assert.throws(() => isolateSyncthingXml(value, 42001, 42002), { code: 'INVALID_SYNCTHING_CONFIG' });
});
test('shutdown budget covers pinned connection/database waits with bounded margin', () => {
  assert.equal(SYNCTHING_STOP_BUDGET_MS, 10000 + 10000 + 5000);
});
