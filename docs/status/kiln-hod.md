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

Last numbers (2026-11-18, WP-1562, aarch64 host, dialects LLVM 19 and
22, both targets, each file with and without `-g`; counts are jobs).
Since WP-1562 the pre-`opt` modules (`alwaysinline`, coroutines) are
offered in process too, so they are in these runs; every compile runs
under the sweep's memory cap (`--mem-limit 2048` here):

| Run | Jobs | Compared | Equal | Refused equally | Not bridged | Different |
|---|---:|---:|---:|---:|---:|---:|
| `bridge-eq tests/codegen` | 5036 | 4442 | 4352 | 90 | 2 | 0 |
| `bridge-eq tests/conformance` | 16102 | 14416 | 14340 | 76 | 12 | 0 |
| `bridge-eq tests/stress` | 116 | 72 | 72 | 0 | 0 | 0 |

Compared modules against WP-1707's runs: 488 more (4442 against 4354 in
tests/codegen, 14488 against 14088 in tests/conformance and
tests/stress), the pre-`opt` modules newly in process. `debug:invalid`
(4 jobs in tests/codegen) is gone: it was the asm-labelled destructor of
WP-1709. Not bridged: `const:splat` (12) and a scalable `alloca` (2).
Not compiled: tests that do not compile to an object (1320 in
tests/conformance, 156 in tests/codegen, 40 in tests/stress) and, at
WP-1562, 4 jobs that hit the memory cap on both routes:
`tests/stress/linkage-pack-dag-36x17.cpp` and
`tests/stress/linkage-type-dag-10x10.cpp` with `-g` (a runaway `-g`
compile in the front end, the same on the stock route). Since WP-1710
(spec/codegen.md "Bounded template names") both compile with `-g` and
compare equal: tests/stress at WP-1710 is 120 jobs, 80 modules compared,
80 equal, none at the memory cap.

`sweep --dual` (with and without `-g`): tests/codegen 5036 jobs (4174
taken in process with the same bytes, 232 declined alike), tests/conformance
16102 (14208, 212), tests/stress 116 (72, 0): 0 object differences, 0
dual mismatches, 0 jobs offered to one route only. hod's declines of
pre-`opt` modules: `alwaysinline:frame-address` (8 jobs) and
`attr:coro_only_destroy_when_complete` (8); both fall back to `opt` and
`llc` with the same bytes as the external route's fallback.

The corpus's run tests through the wrapper (`KILN_BACKEND=hod`, the
commands in kiln_hod/README.md) over tests/conformance, tests/codegen
and tests/stress: 8359 tests, 8199 passed against 8200 with the stock
compiler; the one difference is `driver.crash/ice-parse.cpp` (the crash
report names the compiler source by its absolute path in the wrapper's
build, README). 1905 jobs went to the in-process back end, 1902 compiled
by hod (1880 inputs; 75 of them pre-`opt` modules), 3 declined
(`alwaysinline:frame-address` 2, `a64:atomic-type` 1).

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
| accessor-wide-emit | no | 2263 / 132 | 1113 / 90 | 1141 / 90 | 1070 / 90 |
| accessor-wide-emit | yes | 3613 / 226 | 1413 / 141 | 1358 / 158 | 1314 / 154 |
| stl (below) | no | 329 / 62 | 329 / 37 | 321 / 37 | 324 / 37 |
| stl (below) | yes | 434 / 66 | 379 / 40 | 311 / 40 | 302 / 40 |

The two wrapper columns of the `-g` rows were re-measured at WP-1707
(the bridge now takes `-g` modules; same host, load average 8 to 9;
the other columns are WP-1704's). The `accessor-wide-emit` and `stl` rows
were measured at WP-1562 (kiln at WP-1562's head, hod `d789a92`, release
builds; "stock" is the release wrapper with `KILN_HOD_IN_PROCESS=0`, the
stock compiler; load average 10): both modules have `alwaysinline`
(statistics `triggers`), which until WP-1562 the wrapper left to `opt`
and `llc` (WP-1703's rule) while the stock driver handed them to the
external `hod` unoptimized; now every hod column compiles the module
before `opt` (statistics `backend` `hod`, `in_process` in the wrapper's).
`stl` is a translation unit that includes `<vector>`, `<string>`, `<map>`
and `<algorithm>` and uses each (a sort, a map of strings, `find`,
`to_string`; libstdc++'s `always_inline` functions are its only trigger),
its front end takes most of the time. Its source:

```cpp
#include <algorithm>
#include <map>
#include <string>
#include <vector>

int main() {
  std::vector<int> v;
  for (int i = 0; i < 100; ++i) v.push_back((i * 37) % 101);
  std::sort(v.begin(), v.end());
  std::map<std::string, int> m;
  for (int x : v) m[std::to_string(x % 10)] += x;
  std::string s;
  for (const auto &[k, n] : m) s += k + "=" + std::to_string(n) + ";";
  auto it = std::find(v.begin(), v.end(), 50);
  return static_cast<int>(s.size() + (it != v.end()));
}
```

Command per route: `kiln++ -std=c++26 -freflection -fconstexpr-steps=1000000000 [-g] -O0 -c <unit>`
with `KILN_BACKEND=llvm|hod` (`KILN_HOD=<hod>` for the external route,
`KILN_HOD_IN_PROCESS=0` for the stock columns, `KILN_HOD_BRIDGE=0` for
the text path).

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
