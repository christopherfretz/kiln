# kiln_hod

`kiln` and `kiln++` with the [hod](https://github.com/christopherfretz/hod)
back end linked in: with `-fkiln-backend=hod` an object is compiled inside
the driver's process instead of by a `hod` executable. Without the flag the
binary is the stock compiler. The contract is spec/driver.md, "In-process
back end".

This is a cargo workspace of its own, excluded from the root workspace, so
that the root `Cargo.lock` never names hod and the stock build never
fetches anything from git.

```
kiln_hod/
  Cargo.toml                  the workspace: hod pinned by git revision, lints and profiles as the root's
  Cargo.lock                  its own lock (the root's packages plus hod's crates)
  crates/kiln_hod/            library: `Hod`, the `InProcessBackend` over hod's `compile_object`
  crates/kiln-hod-driver/     binary `kiln` (+ `kiln++` beside it): registers `Hod`, then the stock `main`
    tests/route.rs            programs run, a decline falls back, `--version`, the stock compiler without the flag
    tests/panic.rs            a panic in front of hod is contained; the next compile is hod's again
    tests/mason_meta.rs       `.mason.meta` through hod against the LLVM route
    tests/programs/           the run programs
```

## Building and testing

From the repository root (with `scripts/env.sh` sourced):

```
cargo xtask kiln-hod build [--profile release]   # <target>/kiln_hod/<profile>/kiln++
cargo xtask kiln-hod test                        # the workspace's tests, with the stock kiln++ and hod's executable
cargo xtask kiln-hod sweep tests/codegen         # in process against the external route: same answers, same bytes
cargo xtask kiln-hod sweep --dual tests/codegen  # the same, the bridged object also compared with the text path's
cargo xtask kiln-hod bridge-eq tests/codegen     # the bridge's IR against hod's text reader, module by module
```

The bridge (`crates/kiln_hod/src/bridge/`) copies `kiln_llvm::Module` into
`hod_ir` for every module without debug information; the others take the
text path (spec/driver.md "The `kiln_hod` workspace"). `bridge-eq` is its
specification as a test: what the bridge builds and what `hod_ll` reads from
the printed module must print the same (`hod_ir`'s printer, byte for byte)
and record the same unsupported constructs. A difference fails the run
unless `bridge-eq-expected.txt` lists the file with its reason (it is
empty). `--report <file>` writes one line per job. `KILN_HOD_BRIDGE=0`
turns the bridge off, `KILN_HOD_DUAL=1` compares objects in process, and
`KILN_HOD_BRIDGE_LOG=<file>` logs each module's route.
The last equality numbers and the timing of the four routes are in
[docs/status/kiln-hod.md](../docs/status/kiln-hod.md).

Everything builds `--locked` into `<target>/kiln_hod`; hod's executable for
the sweep is built from the same locked checkout into
`<target>/kiln_hod_hod_cli`. The first build fetches hod from GitHub; with
no network (or a revision that is gone) the xtask says so. CI runs `test`,
`bridge-eq` and the dual codegen sweep in the `kiln-hod` job of `.github/workflows/ci.yml`.

`KILN_HOD_IN_PROCESS=0` leaves the back end unregistered, so one build also
gives the external route for comparisons.

## Moving the pin

1. Set the revision in `Cargo.toml` (`hod = { git = ..., rev = ... }`), and
   `HOD_REV` / `HOD_TAG` in `crates/kiln_hod/src/lib.rs` (a unit test checks
   that the constant, the manifest and the lock agree).
2. `cargo update -p hod --manifest-path kiln_hod/Cargo.toml`.
3. If hod's `EMBED_API` changed, the build fails on the assertion in
   `crates/kiln_hod/src/lib.rs`: adapt `Hod` to the new interface first.
4. `cargo xtask kiln-hod test` and `cargo xtask kiln-hod sweep --dual tests/codegen`.
5. Re-prove the bridge: `cargo xtask kiln-hod bridge-eq tests/codegen
   tests/conformance tests/stress`. The bridge mirrors `hod_ll`'s mapping
   construct by construct, so any change in hod's reader (an attribute, a
   note, the order symbols or attribute sets are made in) shows here as a
   difference; adapt the bridge until there is none.

## Dependencies

The lock adds only hod's own crates to the root's packages (hod, hod_a64,
hod_asm, hod_bc, hod_cg, hod_dwarf, hod_eh, hod_ir, hod_ll, hod_mc_a64,
hod_mc_x64, hod_obj, hod_x64, hod_x86, hod_xform; `MIT OR Apache-2.0`), which
have no dependencies outside hod and forbid `unsafe` code.
