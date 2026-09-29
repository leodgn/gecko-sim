// Assembles a .s with web/assemble.js under Node, as the browser would, and
// writes the binary: CI compares it with the native toolchain's output.
//
// Usage: node tests/assembler-parity.mjs <program.s> <output.bin>

import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const [sourcePath, outputPath] = process.argv.slice(2);

// What assemble.js expects from a browser: the page's base URL (web/, so
// binutils/ resolves to web/binutils/), and fetch() for the JS glue.
globalThis.document = { baseURI: pathToFileURL(path.join(root, "web") + "/").href };
globalThis.fetch = async (url) => ({
  ok: true,
  text: async () => fs.readFileSync(fileURLToPath(url), "utf8"),
});
// Under Node, the Emscripten glue loads the .wasm files with require("fs"),
// relative to __dirname.
globalThis.require = createRequire(import.meta.url);
globalThis.__dirname = path.join(root, "web/binutils");

const { assemble } = await import(pathToFileURL(path.join(root, "web/assemble.js")).href);
const linkerScript = fs.readFileSync(path.join(root, "assets/mmio.ld"), "utf8");
const source = fs.readFileSync(sourcePath, "utf8");

try {
  const binary = await assemble(path.basename(sourcePath), source, linkerScript);
  fs.writeFileSync(outputPath, binary);
} catch (messages) {
  process.stderr.write(String(messages));
  process.exit(1);
}
