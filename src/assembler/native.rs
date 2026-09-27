//! Native `.s` support: assembles a program with the RISC-V GNU toolchain
//! installed on the user's machine.
//!
//! Runs the same pipeline as the course build, in a temporary directory
//! (here for a source file named `gol.s`):
//!
//! ```text
//! <prefix>as -march=rv32i -mabi=ilp32 gol.s -o gol.o
//! <prefix>ld -m elf32lriscv -T mmio.ld gol.o -o gol.elf
//! <prefix>objcopy -O binary gol.elf gol.bin
//! ```
//!
//! The files are named after the user's file so that binutils' messages
//! point at it (`gol.s:12: Error: ...`).
//!
//! The toolchain's prefix depends on how it was installed, so the ones in
//! `TOOLCHAIN_PREFIXES` are tried in order. `ld` always warns that `_start`
//! is missing; that's harmless (execution always starts at `MAIN_BASE`), so
//! only the tools' exit status is checked, not their output.

use std::fs;
use std::path::Path;
use std::process::Command;

/// Prefixes of the RISC-V GNU toolchains we know about, most common first.
/// The first one whose `as` can be run is used for all three tools.
const TOOLCHAIN_PREFIXES: &[&str] = &[
    "riscv64-unknown-elf-", // riscv-gnu-toolchain default, Ubuntu/Debian, Homebrew tap
    "riscv64-elf-",         // Arch, Homebrew core
    "riscv64-linux-gnu-",   // Debian/Ubuntu/Fedora Linux cross toolchain
    "riscv32-unknown-elf-", // riscv-gnu-toolchain built for 32 bits
    "riscv-none-elf-",      // xPack (common on Windows)
    "riscv-none-embed-",    // xPack, pre-2021 name
];

/// The course linker script, embedded at compile time.
const LINKER_SCRIPT: &str = include_str!("../../assets/mmio.ld");

/// Why a `.s` file couldn't be turned into a `.bin`.
#[derive(Debug)]
pub enum AssembleError {
    /// None of the known toolchain prefixes has a runnable `as`.
    ToolchainNotFound { tried: Vec<String> },
    /// A tool (`"as"`, `"ld"` or `"objcopy"`) ran and failed; `stderr` is
    /// its output, verbatim.
    ToolFailed { tool: String, stderr: String },
    /// Reading or writing the temporary files, or starting a tool, failed.
    Io(std::io::Error),
}

impl std::fmt::Display for AssembleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error while assembling: {e}"),
            // binutils' own message already names the file and the line.
            Self::ToolFailed { stderr, .. } => write!(f, "{stderr}"),
            Self::ToolchainNotFound { tried } => write!(
                f,
                "no RISC-V GNU toolchain found (tried prefixes: {}); \
                 install one, or pass an already assembled .bin file",
                tried.join(", ")
            ),
        }
    }
}

impl std::error::Error for AssembleError {}

impl From<std::io::Error> for AssembleError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Assembles and links `source` (the text of a `.s` file) and returns the
/// raw binary image, ready to load at `MAIN_BASE`.
///
/// `file_name` is the name of the file `source` was read from, e.g.
/// `gol.s`; it only appears in error messages. Any directory part is
/// ignored.
///
/// # Errors
///
/// See `AssembleError`: no toolchain installed, an assembler or linker
/// error in the program, or an I/O error.
pub fn assemble(file_name: &str, source: &str) -> Result<Vec<u8>, AssembleError> {
    assemble_with(TOOLCHAIN_PREFIXES, file_name, source)
}

/// `assemble`, with the list of toolchain prefixes to try as a parameter,
/// so tests can simulate a missing toolchain.
fn assemble_with(
    prefixes: &[&str],
    file_name: &str,
    source: &str,
) -> Result<Vec<u8>, AssembleError> {
    let prefix = prefixes
        .iter()
        .find(|prefix| is_installed(prefix))
        .ok_or_else(|| AssembleError::ToolchainNotFound {
            tried: prefixes.iter().map(|p| p.to_string()).collect(),
        })?;

    // Keep only the last path component: the file is created inside the
    // temporary directory, and `../x.s` must not escape it.
    let file_name = Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("program.s");
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("program");
    let object = format!("{stem}.o");
    let elf = format!("{stem}.elf");
    let bin = format!("{stem}.bin");

    // Deleted with its content when `dir` is dropped, on every return path.
    let dir = tempfile::tempdir()?;

    fs::write(dir.path().join(file_name), source)?;
    fs::write(dir.path().join("mmio.ld"), LINKER_SCRIPT)?;

    run(
        prefix,
        "as",
        &["-march=rv32i", "-mabi=ilp32", file_name, "-o", &object],
        dir.path(),
    )?;

    run(
        prefix,
        "ld",
        &["-m", "elf32lriscv", "-T", "mmio.ld", &object, "-o", &elf],
        dir.path(),
    )?;

    run(prefix, "objcopy", &["-O", "binary", &elf, &bin], dir.path())?;

    Ok(fs::read(dir.path().join(&bin))?)
}

/// Returns whether `<prefix>as` exists and can be started.
fn is_installed(prefix: &str) -> bool {
    Command::new(format!("{prefix}as"))
        .arg("--version")
        .output()
        .is_ok()
}

/// Runs `<prefix><tool> <args>` from `dir`.
///
/// # Errors
///
/// `ToolFailed` with the tool's stderr if it exits with a failure status,
/// `Io` if it can't be started.
fn run(prefix: &str, tool: &str, args: &[&str], dir: &Path) -> Result<(), AssembleError> {
    let output = Command::new(format!("{prefix}{tool}"))
        .args(args)
        .current_dir(dir)
        .output()?;

    if output.status.success() {
        return Ok(());
    }

    Err(AssembleError::ToolFailed {
        tool: tool.to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::CpuRunner;

    #[test]
    fn assembles_a_single_instruction() {
        let bin = assemble("one.s", "addi x1, x0, 5\n").unwrap();

        // addi x1, x0, 5 = 0x00500093, little-endian.
        assert_eq!(bin, vec![0x93, 0x00, 0x50, 0x00]);
    }

    /// Uses what the Rust assemblers we tried couldn't handle (block
    /// comments, `.equ`, constant expressions, sections, `li`/`la`), then
    /// runs the result: it must also be laid out where the emulator expects
    /// it (code at `MAIN_BASE`, `.data` on the next 4 KiB page).
    #[test]
    fn assembled_program_runs_on_the_emulator() {
        let source = "
/* Shows a word from .data on SEVEN_SEGS. */
.equ SEVEN_SEGS, 0x60000000
.equ OFFSET, 2 * 4

.section .text
    li   t0, SEVEN_SEGS     # li: lui + addi
    la   t1, table          # la: auipc + addi
    lw   t2, OFFSET - 4(t1)
    sw   t2, 0(t0)
end:
    j    end

.section .data
table:
    .word 0
    .word 0x3f063f06
";
        let bin = assemble("seven_segs.s", source).unwrap();
        let mut runner = CpuRunner::new(&bin).unwrap();
        let state = runner.board_state();

        runner.run_batch().unwrap();

        assert_eq!(state.lock().unwrap().seven_segs, 0x3f063f06);
    }

    #[test]
    fn syntax_error_reports_the_assembler_output() {
        let err = assemble("gol.s", "addi x1, x0\n").unwrap_err();

        match &err {
            AssembleError::ToolFailed { tool, stderr } => {
                assert_eq!(tool, "as");
                assert!(
                    stderr.contains("gol.s:1: Error: illegal operands"),
                    "stderr should be binutils' own message, got: {stderr}"
                );
            }
            other => panic!("expected ToolFailed, got {other:?}"),
        }
        assert!(
            err.to_string().contains("Error: illegal operands"),
            "Display must show the assembler's message"
        );
    }

    #[test]
    fn undefined_label_is_reported_by_the_linker() {
        let err = assemble("gol.s", "j nowhere\n").unwrap_err();

        match &err {
            AssembleError::ToolFailed { tool, stderr } => {
                assert_eq!(tool, "ld");
                assert!(
                    stderr.contains("undefined reference to `nowhere'"),
                    "got: {stderr}"
                );
                assert!(
                    stderr.contains("gol.o"),
                    "the object file must be named after the source, got: {stderr}"
                );
            }
            other => panic!("expected ToolFailed, got {other:?}"),
        }
    }

    #[test]
    fn directory_part_of_the_file_name_is_ignored() {
        let err = assemble("../../somewhere/gol.s", "addi x1, x0\n").unwrap_err();

        match &err {
            AssembleError::ToolFailed { stderr, .. } => {
                assert!(stderr.starts_with("gol.s:"), "got: {stderr}");
            }
            other => panic!("expected ToolFailed, got {other:?}"),
        }
    }

    #[test]
    fn missing_toolchain_lists_the_prefixes_tried() {
        let err = assemble_with(&["no-such-toolchain-"], "one.s", "addi x1, x0, 5\n").unwrap_err();

        match &err {
            AssembleError::ToolchainNotFound { tried } => {
                assert_eq!(tried, &vec!["no-such-toolchain-".to_string()]);
            }
            other => panic!("expected ToolchainNotFound, got {other:?}"),
        }
        let message = err.to_string();
        assert!(message.contains("no-such-toolchain-"), "got: {message}");
        assert!(
            message.contains(".bin"),
            "should suggest passing a .bin instead, got: {message}"
        );
    }
}
