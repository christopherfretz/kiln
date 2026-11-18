# The `kiln_hod` workspace: equality and timing

`kiln_hod/` is a separate cargo workspace (its own lock, hod by git
revision) that builds a `kiln++` with the hod back end linked in
(spec/driver.md "The `kiln_hod` workspace"; WP-1703). Since WP-1704 a
module reaches hod through the *bridge*: `kiln_llvm::Module` is copied
into `hod_ir` and fed to a `hod::Session`, never printed; since WP-1707
that includes modules with debug information (`-g`). A module the bridge
does not map takes the *text path* (printed, read by hod's text reader).

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

Last numbers (2026-11-18, WP-1707, aarch64 host, dialects LLVM 19 and
22, both targets, each file with and without `-g`; counts are jobs):

| Run | `-g` | Compared | Equal | Refused equally | Different |
|---|---|---:|---:|---:|---:|
| `bridge-eq tests/codegen` | no | 2318 | 2282 | 36 | 0 |
| `bridge-eq tests/codegen` | yes | 2036 | 1994 | 42 | 0 |
| `bridge-eq tests/conformance tests/stress` | no | 7052 | 7014 | 38 | 0 |
| `bridge-eq tests/conformance tests/stress` | yes | 7036 | 6996 | 40 | 0 |

(A file whose own arguments have `-g` runs once, in the `no` row.)
Refused equally, by first construct, over both runs:
`attr:speculative_load_hardening` 44, `alloca:scalable` 26,
`debug:DW_OP_bregx` 28 (SVE locals), `attr:nonlazybind` 16,
`attr:llvm.assume` 8, `global-metadata:callback` 8,
`attr:aarch64_pstate_sm_enabled` 8, `attr:fn_ret_thunk_extern` 8,
`debug:invalid` 6, `attr:no_caller_saved_registers` 4. Left to the text
path: `splat` constants (12 jobs) and a scalable `alloca` the bridge
does not type (2, `debug_sve.cpp`); no module for its debug information.

`sweep --dual tests/codegen tests/conformance` (with and without `-g`):
21138 jobs, 0 object differences, 0 dual mismatches; 17932 taken in
process with the same bytes, 414 declined alike; the wrapper's modules:
18340 bridged (`-g` included), 6 text path (`splat`).

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
| hello | yes | 392 / 63 | 378 / 51 | 376 / 50 | 376 / 50 |
| registration-wide-obj | no | 2932 / 233 | 1106 / 125 | 1072 / 148 | 979 / 145 |
| registration-wide-obj | yes | 5464 / 594 | 1820 / 342 | 1628 / 427 | 1543 / 392 |
| trial-folds | no | 2538 / 179 | 1375 / 177 | 1354 / 179 | 1259 / 178 |
| trial-folds | yes | 3982 / 295 | 1896 / 242 | 1814 / 245 | 1671 / 245 |
| accessor-wide-emit | no | 1990 / 137 | 972 / 91 | 2059 / 137 (llvm) | 2097 / 137 (llvm) |
| accessor-wide-emit | yes | 3140 / 232 | 1262 / 147 | 3099 / 233 (llvm) | 3068 / 233 (llvm) |

The two wrapper columns of the `-g` rows were re-measured at WP-1707
(the bridge now takes `-g` modules; same host, load average 8 to 9;
the other columns are WP-1704's). `accessor-wide-emit` has `alwaysinline`: the wrapper
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
| registration-wide-obj, `-g` | text | 291.1 | 355.3 | 403.0 | 408.4 | | |
| registration-wide-obj, `-g` | bridge | | 226.6 | 283.5 | 294.0 | 92.3 | 82.9 |
| trial-folds, `-g` | text | 189.2 | 231.4 | 253.9 | 257.3 | | |
| trial-folds, `-g` | bridge | | 148.1 | 173.9 | 177.9 | 57.8 | 42.5 |

On the bridge no module is printed: the statistics line has no
`ir_bytes` and `print_ir_ms` is 0 for in-process jobs on both paths
(spec/driver.md "Compile statistics log"). Peak RSS of the largest unit
without `-g`: 145 MB on the bridge, 148 MB on the text path; with `-g`:
392 MB on the bridge, 427 MB on the text path.
