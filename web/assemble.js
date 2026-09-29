// Assembles a `.s` file in the browser with GNU binutils compiled to
// WebAssembly (see binutils/README.md): as -> ld -> objcopy, the same
// pipeline as the native build (src/assembler/native.rs).
//
// Each tool runs in its own Emscripten instance, with an in-memory file
// system: nothing leaves the browser. The tools are downloaded on the
// first call only, then cached.
//
// Called from Rust (src/assembler/web.rs) through wasm-bindgen.

// Trunk copies web/binutils/ to binutils/ next to the page.
const BINUTILS = new URL("binutils/", document.baseURI).href;

const GLUE = {
  as: "riscv64-linux-gnu-as.js",
  ld: "riscv64-linux-gnu-ld.js",
  objcopy: "riscv64-linux-gnu-objcopy.js",
};

// One Emscripten module factory per tool, loaded once.
const factories = {};

// The glue files are CommonJS scripts (`module.exports = factory`), not ES
// modules: evaluate them with a `module` object to get the factory.
async function loadFactory(tool) {
  if (!factories[tool]) {
    const response = await fetch(BINUTILS + GLUE[tool]);
    if (!response.ok) {
      throw `Could not load the assembler (${GLUE[tool]}: HTTP ${response.status})`;
    }
    let text = await response.text();
    // A `#!` line is only valid at the very start of a file, not inside a
    // function body.
    if (text.startsWith("#!")) text = text.slice(text.indexOf("\n"));
    const module = { exports: {} };
    new Function("module", "exports", text)(module, module.exports);
    factories[tool] = module.exports;
  }
  return factories[tool];
}

// Runs `tool args...` on a fresh instance, with `inputs` ({ name: content })
// written to its file system first. Resolves to the instance's file system;
// rejects with the tool's stderr if it fails.
async function run(tool, args, inputs) {
  let stderr = "";
  const factory = await loadFactory(tool);
  const instance = await factory({
    noInitialRun: true,
    thisProgram: tool,
    locateFile: (path) => BINUTILS + path,
    print: () => {},
    printErr: (line) => {
      stderr += line + "\n";
    },
  });
  for (const [name, content] of Object.entries(inputs)) {
    instance.FS.writeFile(name, content);
  }
  let status;
  try {
    status = instance.callMain(args);
  } catch (exit) {
    status = exit.status ?? 1;
  }
  if (status !== 0) throw stderr;
  return instance.FS;
}

// Assembles and links `source` (the text of `fileName`, e.g. "gol.s") with
// `linkerScript`, and resolves to the raw binary image as a Uint8Array.
// Rejects with binutils' messages as a string. Intermediate files are named
// after `fileName`, so messages point at it ("gol.s:12: Error: ...").
export async function assemble(fileName, source, linkerScript) {
  const stem = fileName.replace(/\.[^.]*$/, "");
  const [object, elf, bin] = [`${stem}.o`, `${stem}.elf`, `${stem}.bin`];

  let fs = await run("as", ["-march=rv32i", "-mabi=ilp32", fileName, "-o", object], {
    [fileName]: source,
  });
  fs = await run("ld", ["-m", "elf32lriscv", "-T", "mmio.ld", object, "-o", elf], {
    [object]: fs.readFile(object),
    "mmio.ld": linkerScript,
  });
  fs = await run("objcopy", ["-O", "binary", elf, bin], {
    [elf]: fs.readFile(elf),
  });
  return fs.readFile(bin);
}
