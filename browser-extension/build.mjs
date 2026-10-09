import { mkdir, readFile, writeFile, rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";
const root = new URL("./", import.meta.url);
const dist = new URL("./dist/", root);
await rm(dist, { recursive: true, force: true });
await mkdir(dist, { recursive: true });
const files = [
  "manifest.json",
  "icon16.png",
  "icon48.png",
  "icon128.png",
  "worker.js",
  "fill.js",
  "popup.html",
  "popup.css",
  "popup.js",
  "policy.js",
  "i18n.js",
  "options.html",
  "options.css",
  "options.js",
];
for (const file of files)
  await writeFile(new URL(file, dist), await readFile(new URL(file, root)));
console.log(`Unpacked Chrome extension: ${fileURLToPath(dist)}`);
