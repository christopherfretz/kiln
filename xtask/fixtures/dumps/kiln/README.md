`expect: layout` and `expect: vtables` fixtures for the kiln path, run with
`--compiler xtask/fixtures/fake-kiln/kiln` (oracle clang++-19). The fake kiln
prints the canned dump `<test>.layout` for `-fdump-record-layouts` and
`<test>.vtables` for `-fdump-vtable-layouts`; the harness compares it with
clang's `-fdump-record-layouts-complete` (block by block) or emission-time
`-fdump-vtable-layouts` (as a set of blocks). Outcome = file-name prefix, as
in `../../basic/`. With `--no-oracle` every test here is a SKIP.
