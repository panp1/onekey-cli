import "./build.mjs";
import { readFile, writeFile, readdir } from "node:fs/promises";
import { deflateRawSync } from "node:zlib";
import { createHash } from "node:crypto";
const root = new URL("./", import.meta.url);
const manifest = JSON.parse(
  await readFile(new URL("manifest.json", root), "utf8"),
);
function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}
async function zip(files, output) {
  let offset = 0;
  const local = [],
    central = [];
  for (const [name, url] of files) {
    const bytes = await readFile(url),
      compressed = deflateRawSync(bytes),
      filename = Buffer.from(name),
      crc = crc32(bytes);
    const header = Buffer.alloc(30);
    header.writeUInt32LE(0x04034b50);
    header.writeUInt16LE(20, 4);
    header.writeUInt16LE(8, 6);
    header.writeUInt16LE(8, 8);
    header.writeUInt16LE(33, 12);
    header.writeUInt32LE(crc, 14);
    header.writeUInt32LE(compressed.length, 18);
    header.writeUInt32LE(bytes.length, 22);
    header.writeUInt16LE(filename.length, 26);
    local.push(header, filename, compressed);
    const entry = Buffer.alloc(46);
    entry.writeUInt32LE(0x02014b50);
    entry.writeUInt16LE(20, 4);
    entry.writeUInt16LE(20, 6);
    entry.writeUInt16LE(8, 8);
    entry.writeUInt16LE(8, 10);
    entry.writeUInt16LE(33, 14);
    entry.writeUInt32LE(crc, 16);
    entry.writeUInt32LE(compressed.length, 20);
    entry.writeUInt32LE(bytes.length, 24);
    entry.writeUInt16LE(filename.length, 28);
    entry.writeUInt32LE(offset, 42);
    central.push(entry, filename);
    offset += header.length + filename.length + compressed.length;
  }
  const directory = Buffer.concat(central),
    end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50);
  end.writeUInt16LE(files.length, 8);
  end.writeUInt16LE(files.length, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  const bytes = Buffer.concat([...local, directory, end]);
  await writeFile(new URL(output, root), bytes);
  return `${createHash("sha256").update(bytes).digest("hex")}  ${output}`;
}
const runtime = (await readdir(new URL("dist/", root)))
  .sort()
  .map((name) => [name, new URL(`dist/${name}`, root)]);
const version = manifest.version;
const checksums = [];
checksums.push(await zip(runtime, `onekey-browser-store_${version}.zip`));
checksums.push(
  await zip(
    [
      ...runtime.map(([name, url]) => [`extension/${name}`, url]),
      ...["install.sh", "install.ps1", "README.md", "PRIVACY.md"].map(
        (name) => [name, new URL(name, root)],
      ),
    ],
    `onekey-browser_${version}.zip`,
  ),
);
await writeFile(new URL("checksums.txt", root), checksums.join("\n") + "\n");
console.log(checksums.join("\n"));
