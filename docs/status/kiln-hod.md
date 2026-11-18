# The `kiln_hod` workspace: equality and timing

`kiln_hod/` is a separate cargo workspace (its own lock, hod by git
revision) that builds a `kiln++` with the hod back end linked in
(spec/driver.md "The `kiln_hod` workspace"; WP-1703). Since WP-1704 a
module without debug information reaches hod through the *bridge*:
`kiln_llvm::Module` is copied into `hod_ir` and fed to a `hod::Session`,
never printed. Every other module takes the *text path* (printed, read by
hod's text reader).

## How the bridge is checked

- **IR equality** (`cargo xtask kiln-hod bridge-eq <dir>...`): for each
  module, `hod_ir`'s print of what the bridge builds against the print of
  what `hod_ll` reads from the printed module, byte for byte, and the
  unsupported constructs both record. Expected differences:
  `kiln_hod/bridge-eq-expected.txt` (empty).
- **Object equality** (`cargo xtask kiln-hod sweep [--dual] <dir>...`):
  the in-process objects against the stock `kiln++` driving the external
  `hod` of the same revision; `--dual` also has the wrapper compile each
  bridged module on the text path and compare the objects.

Last numbers (2026-11-18, aarch64 host, dialects LLVM 19 and 22, both
targets; counts are jobs):

| Run | Compared | Equal | Refused equally | Different |
|---|---:|---:|---:|---:|
| `bridge-eq tests/codegen` | 2032 | 1996 | 36 | 0 |
| `bridge-eq tests/conformance tests/stress` | 7040 | 7002 | 38 | 0 |

Refused equally, by first construct: `attr:speculative_load_hardening`
22, `alloca:scalable` 26, `attr:nonlazybind` 8, `attr:llvm.assume` 4,
`global-metadata:callback` 4, `attr:aarch64_pstate_sm_enabled` 4,
`attr:fn_ret_thunk_extern` 4, `attr:no_caller_saved_registers` 2. Left
to the text path: debug information (300 jobs), `splat` constants (6).
The four `conformance` jobs that first differed (`module asm` with
several lines) were fixed and re-run equal.

`sweep --dual tests/codegen tests/conformance` (with and without `-g`):
21138 jobs, 0 object differences, 0 dual mismatches; 17932 taken in
process with the same bytes, 414 declined alike; the wrapper's modules:
9015 bridged, 9325 text path (debug), 6 text path (`splat`).

## Timing

Fastest of 5 runs per row, wall clock of the whole `kiln++ -std=c++26
-freflection -O0 -c` (milliseconds), peak RSS (`/usr/bin/time -v`, MB).
Host: aarch64, 24 cores, shared with other jobs (load average 8 to 10).
Revisions: kiln at WP-1704, hod `d789a92`, release builds;
`llc` is LLVM 22.1.8. Routes: stock `kiln++` with `llc`; stock with the
external `hod`; the wrapper's text path (`KILN_HOD_BRIDGE=0`); the
wrapper's bridge. `hello` is `#include <iostream>` and one `std::cout`
line; the others are the three `bench/` units largest in IR (`ir_bytes`
at `-O0` without `-g`: 26.7, 19.6 and 13.6 MB).

| Unit | `-g` | stock + `llc` | stock + `hod` | wrapper, text | wrapper, bridge |
|---|---|---:|---:|---:|---:|
| hello | no | 381 / 62 | 371 / 50 | 374 / 51 | 370 / 52 |
| hello | yes | 392 / 63 | 378 / 51 | 372 / 51 | 381 / 53 (text) |
| registration-wide-obj | no | 2932 / 233 | 1106 / 125 | 1072 / 148 | 979 / 145 |
| registration-wide-obj | yes | 5464 / 594 | 1820 / 342 | 1770 / 402 | 1750 / 402 (text) |
| trial-folds | no | 2538 / 179 | 1375 / 177 | 1354 / 179 | 1259 / 178 |
| trial-folds | yes | 3982 / 295 | 1896 / 242 | 1831 / 243 | 1835 / 245 (text) |
| accessor-wide-emit | no | 1990 / 137 | 972 / 91 | 2059 / 137 (llvm) | 2097 / 137 (llvm) |
| accessor-wide-emit | yes | 3140 / 232 | 1262 / 147 | 3098 / 233 (llvm) | 3090 / 232 (llvm) |

With `-g` the bridge column is the text path (the module has debug
information). `accessor-wide-emit` has `alwaysinline`: the wrapper
leaves it to `opt` and `llc` (WP-1703's rule for in-process modules),
while the stock driver hands it to the external `hod` unoptimized.

hod's own times on the two bridged units (milliseconds, from
`KILN_HOD_BRIDGE_LOG`: the back end's call from its start; on the text
path `parse` includes printing the module):

| Unit | Path | parse ends | bodies end | object ends | total | `Report.bridge` | `Report.session` |
|---|---|---:|---:|---:|---:|---:|---:|
| registration-wide-obj | text | 72.9 | 202.4 | 255.5 | 260.3 | | |
| registration-wide-obj | bridge | | 100.5 | 155.0 | 158.8 | 60.8 | 75.4 |
| trial-folds | text | 46.9 | 149.2 | 187.1 | 189.0 | | |
| trial-folds | bridge | | 55.6 | 94.5 | 95.7 | 33.5 | 54.1 |
| hello | text | 1.0 | 1.5 | 2.5 | 2.6 | | |
| hello | bridge | | 0.9 | 1.8 | 1.8 | 0.02 | 1.0 |

On the bridge no module is printed: the statistics line has no
`ir_bytes` and `print_ir_ms` is 0 for in-process jobs on both paths
(spec/driver.md "Compile statistics log"). Peak RSS of the largest unit
without `-g`: 145 MB on the bridge, 148 MB on the text path.
