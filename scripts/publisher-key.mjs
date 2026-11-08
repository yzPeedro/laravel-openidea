import { generateKeyPairSync } from "node:crypto";
import { existsSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve, isAbsolute } from "node:path";
import { fileURLToPath } from "node:url";

const target = process.argv[2];
if (!target) {
  console.error("usage: node scripts/publisher-key.mjs <key-file outside the repository>");
  process.exit(1);
}

const file = resolve(target);
const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const inside = relative(repo, file);
if (!inside.startsWith("..") && !isAbsolute(inside)) {
  console.error(`${file} is inside the repository: keep the private key somewhere it can't be committed`);
  process.exit(1);
}
if (existsSync(file)) {
  console.error(`${file} already exists`);
  process.exit(1);
}

const { privateKey, publicKey } = generateKeyPairSync("ed25519");
const seed = privateKey.export({ format: "der", type: "pkcs8" }).subarray(-32);
const pub = publicKey.export({ format: "der", type: "spki" }).subarray(-32);

writeFileSync(file, seed.toString("base64"), { mode: 0o600, flag: "wx" });
console.log(`private key written to ${file}`);
console.log(`public key: ${pub.toString("base64")}`);
