# GNU binutils for the browser

`as`, `ld` and `objcopy` from [GNU binutils](https://www.gnu.org/software/binutils/)
2.46.0, configured for `riscv64-linux-gnu` and compiled to WebAssembly
with Emscripten. `web/assemble.js` runs them to assemble `.s` files in the
browser.

- `riscv64-linux-gnu-{as,ld,objcopy}.js`: Emscripten JS glue.
- `as-new.wasm`, `ld-new.wasm`, `objcopy.wasm`: the programs.

## Origin

Copied unmodified from
[racerxdl/riscv-online-asm](https://github.com/racerxdl/riscv-online-asm)
at commit `0bc309eb8da8c2573e8f122067c52b815f13090c` (`js/` folder).
They are built by that repository's
[`build.sh`](https://github.com/racerxdl/riscv-online-asm/blob/0bc309eb8da8c2573e8f122067c52b815f13090c/build.sh)
from the unmodified binutils 2.46.0 release.

## Licenses

- GNU binutils is free software under the GNU General Public License,
  version 3: see [`COPYING`](COPYING). Its complete corresponding source
  is the binutils 2.46.0 release,
  <https://ftp.gnu.org/gnu/binutils/binutils-2.46.0.tar.xz>, built with
  the `build.sh` linked above.
- The build script and wrapper from riscv-online-asm are under the MIT
  license, Copyright (c) 2026 Lucas Teske: see
  [`LICENSE-riscv-online-asm`](LICENSE-riscv-online-asm).

These programs run as separate programs next to gecko-sim; gecko-sim's
own license does not apply to them.
