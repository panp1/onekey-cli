import assert from "node:assert/strict";
import { beforeEach, test } from "node:test";
import {
  ALL_HTTPS,
  hasAllHttpsAccess,
  setAllHttpsAccess,
  removeUnusedHostPermissions,
} from "./host-permissions.js";

let granted, requests, removals;
beforeEach(() => {
  granted = new Set();
  requests = [];
  removals = [];
  globalThis.chrome = {
    permissions: {
      contains: async ({ origins }) =>
        origins.every((origin) => granted.has(origin)),
      getAll: async () => ({ origins: [...granted] }),
      request: async ({ origins }) => {
        requests.push(origins);
        origins.forEach((origin) => granted.add(origin));
        return true;
      },
      remove: async ({ origins }) => {
        removals.push(origins);
        origins.forEach((origin) => granted.delete(origin));
        return true;
      },
    },
  };
});
test("global access is opt-in and requests only HTTPS directly from the gesture", async () => {
  assert.equal(await hasAllHttpsAccess(), false);
  assert.deepEqual(requests, []);
  const request = setAllHttpsAccess(true);
  assert.deepEqual(requests, [["https://*/*"]]);
  assert.equal(await request, true);
  assert.equal(await hasAllHttpsAccess(), true);
});
test("denied permission does not enable global access", async () => {
  chrome.permissions.request = async () => false;
  assert.equal(await setAllHttpsAccess(true), false);
  assert.equal(await hasAllHttpsAccess(), false);
});
test("revoking global access does not request new access or remove loopback grants", async () => {
  granted.add(ALL_HTTPS);
  granted.add("http://localhost/*");
  await setAllHttpsAccess(false);
  assert.equal(await hasAllHttpsAccess(), false);
  assert(granted.has("http://localhost/*"));
  assert.deepEqual(removals, [[ALL_HTTPS]]);
  assert.deepEqual(requests, []);
});
test("account cleanup preserves deliberate global access while removing unused specific hosts", async () => {
  for (const origin of [
    ALL_HTTPS,
    "https://login.example.com/*",
    "https://sso.example.com/*",
    "https://old.example.com/*",
  ])
    granted.add(origin);
  await removeUnusedHostPermissions([
    {
      enabled: true,
      origin: "https://login.example.com",
      loginOrigins: ["https://sso.example.com"],
    },
    { enabled: false, origin: "https://old.example.com" },
  ]);
  assert.equal(await hasAllHttpsAccess(), true);
  assert.deepEqual(removals, [["https://old.example.com/*"]]);
  await removeUnusedHostPermissions([]);
  assert.deepEqual([...granted], [ALL_HTTPS]);
});
test("external browser revocation is reflected without a stored preference", async () => {
  await setAllHttpsAccess(true);
  granted.delete(ALL_HTTPS);
  assert.equal(await hasAllHttpsAccess(), false);
});
