# PCH snapshot: gates

WP-1395, 2026-11-13. These are the gates for the snapshot PCH product
(spec/pch.md "Consumer"). WP-1395 ran them with the
then hidden `-fkiln-pch-snapshot=write|load`; since WP-1404 the snapshot
is the default, and "Default path (WP-1404)" below re-runs them with no
option on either side. The tables are
from the tree that ships: main with the single-parse producer and
streaming decode (WP-1393) and this package, `ba5d00e5b` plus the one
harness fix below (runs 5 and libc++ second pair; runs 1 to 4 and the
first libc++ pair were on the tree before WP-1393). They are:

- the restore/replay accounting of `cargo xtask test --pch-diff
  snapshot`, and `--pch-diff-require-restore` with its allow-list
  (`tests/pch-snapshot-replay-allow.txt`);
- a whole-corpus run, with the synthetic `<bits/stdc++.h>` prefix for
  tests that have no `#include`;
- the libc++ suite with snapshots against a no-PCH baseline;
- a CI lane in `cargo xtask ci`.

Contracts: spec/harness.md "PCH differential", "Snapshot accounting"
and "Synthetic prefix"; spec/libcxx.md `--pch-diff`.

**Summary.**
- Every corpus test the differential can compare restores, except one
  that is on the allow-list. No snapshot output differs from the
  test's own output without a PCH.
- One snapshot bug was found and fixed: the global module fragment's
  dedup of the prefix's declarations.
- libc++ (llvmorg-19.1.7; std/utilities, containers, algorithms,
  ranges, strings, language.support): 3894 tests compiled with a
  snapshot PCH, all 3894 restored, 0 replayed, and 0 outcome
  differences from the no-PCH baseline of the same session, on both
  trees.

## Accounting

With `--pch-diff snapshot`, every consumer's path is read from the
`snapshot-timing: note:` line the driver prints when
`KILN_SNAP_TIMING=1` is set (spec/driver.md). After the summary the
run prints the paths and the skips:

```
pch-diff snapshot: N consumers ran: R restored, P replayed; F failed before the consumer ran; S skipped
      P  replay: <producer: or consumer:> <reason, numbers as N, paths as '…'>
      R  snapshot (k failed)
  skipped:
      S  <reason>
```

`--pch-diff-require-restore` makes a consumer that replays FAIL unless
its reason matches an allow-list entry (`<pattern> :: <justification>`).
The CI lane runs with it, so a test that used to restore and now
replays is a regression there.

### Refusals and what they mean for a user

A refusal is never an error. The consumer replays the header, which is
slower but gives the same result, and `-v` says why. These are the
reasons the consumer or producer gives (spec/driver.md):

- **`consumer: the command line differs: '<a>' (producer) vs '<b>'
  (here)`.** The PCH was built with other options than the ones used
  now, so it is replayed. Seen 16 times in the first corpus run, all a
  harness artefact (below). For a user: build the PCH with the same
  options as the files that use it. Options that cannot change the
  prefix (`-o`, `-c`, `-M*`, `-v`, the input and the like) are not
  compared.
- **`consumer: '<file>' is a module unit, which a replay compiles with
  the header before its module declaration`.** A PCH used by a module
  unit (`module m;` with no global module fragment) puts the header
  before the module declaration. That is a different program from the
  header inside a global module fragment, so the snapshot is not used.
  Seen once, with the synthetic prefix only (allowed). For a user: put
  the `#include`s in the global module fragment instead.
- **`producer: Parser.mods: not at rest`.** The header ends in an open
  global module fragment (`module;`). The PCH has no snapshot. Not seen
  in the corpus.
- **`consumer: the header reported N warning(s) and error(s)` under
  `-verify`, or a file of the header holds a `-verify` prefix.** The
  verifier would need the header's diagnostics. Not seen in the corpus
  (the `-verify` tests are not compared).
- **`consumer: '<file>' changed`.** A header changed after the PCH was
  built and `-fno-validate-pch` let it through. Not seen.

## Whole corpus

`cargo xtask test --compiler <harness kiln++> --pch-diff snapshot
--pch-diff-require-restore` over all 7045 tests (8 jobs). The runs
follow one another; each run has the previous run's fixes.

| run | pass | fail | skip | restored | replayed | time |
|---|---|---|---|---|---|---|
| 1: before the fixes | 904 | 6 | 6135 | 894 | 16 | 174 s |
| 2: `ir` without an oracle compared | 957 | 16 | 6072 | 957 | 16 | 424 s |
| 3: `--pch-diff-synthetic` | 5684 | 3 (+1 killed) | 1357 | 5686 | 1 | 3212 s |
| 4: after the fixes, before WP-1393 | 5686 | 0 | 1361 | 5685 | 1 (allowed) | 3326 s |
| 5: shipping tree (with WP-1393) | 5694 | 0 (+1 killed, now a skip) | 1360 | 5693 | 1 (allowed) | 3835 s |

Runs 4 and 5 are the same command as run 3 after the fixes below (7047
and 7055 tests). Run 5's one KILLED is `contracts-many.cpp` again, now
on the PCH build rather than the reference (below); with the fix it is a
skip, so run 5 is 5693 consumers, 5692 restored, 1 allowed replay, 0
failures. Run 5's accounting as printed:

```
pch-diff snapshot: 5694 consumers ran: 5693 restored, 1 replayed; 1 failed before the consumer ran; 1360 skipped
      1  replay: consumer: '…' is a module unit, which a replay compiles with the header before its module declaration
   5693  snapshot
```

### Coverage of each prefix rule

- **Leading `#include`s** (spec/harness.md "PCH differential"): the
  test's leading `#include`s form the PCH. 973 tests in run 2.
- **Synthetic `<bits/stdc++.h>`** (`--pch-diff-synthetic`, a test with
  no `#include` at all): the PCH is `#include <bits/stdc++.h>`, one
  per set of flags, shared by the tests. It is used only when the test,
  given the header as an `-include`, reports exactly what it reports
  alone (success, diagnostics, run output). Run 5's other consumers,
  about 4720 (5693 less the 973; run 5 has 10 tests more than run 2). It is not used
  for:
  - 253 tests whose result the header changes. In a sample: the test
    defines its own `std::nothrow_t` or `std::destroying_delete_t`
    (a redefinition with the header); it names `atomic` and the
    header's `std::atomic` is found instead; its constexpr budget
    flags are too small for the header's own constants;
  - 12 whose flags do not build the header (a second source file in
    `args:`; `-mgeneral-regs-only` on x86);
  - 1 that kiln fails with the header (`redecl-chain-65536.cpp`);
  - 1 whose memory cap (256 MiB) is sized for the test alone;
  - 2 stack-guard tests: where the stack guard fires depends on how
    deep the compiler's frames are, which a restore shifts by a few
    frames;
  - 103 that have an `#include` that is not leading.

The remaining skips are tests the differential does not compare at all
(`expect: ast`, `pp`, `cfg`, `layout`, `vtables`, `debugger`, `ice`:
661; `pch:`/`unit:` lines: 20; not C++: 239; target or host
requirements: 20; leading `#include`s that do not compile: 18).

### Triage of every failure

Classes: (a) snapshot bug, (b) replay bug, (c) harness artefact.

- **(a) `module.interface/*` (3 tests, in the CI slice): "template
  parameter redefines default argument" after a restore.** The parser
  dedups the declarations a global module fragment repeats from a
  textual prefix by their token range (spec/modules.md §6). Those
  records were made while the prefix was parsed and were not in the
  snapshot, so a GMF that included the prefix's header again declared
  everything a second time. Fixed: the records are made into Sema's
  `ModSema::prefix_keys`, which is snapshot state, and the parser takes
  them back on restore (`kiln_sema/src/modules/mod.rs`,
  `kiln_parse/src/modules.rs`). Regression test: kiln_driver
  `tests/pch_snapshot.rs` `module_fragment_repeats_the_prefix`. Before
  the fix, the harness's producer did not get the tests' module flags,
  so these consumers replayed and the bug was hidden; the producer now
  mirrors the consumer (deliverable 2).
- **(c) 6 FAILs in run 1, the same under replay.** Each was a
  comparison of things a PCH changes by design:
  - `!srcloc` of inline asm (a value per location);
  - the `DIFile` checksum of the main file, which is the copy without
    its leading `#include`s;
  - ASan reports (addresses, pid, BuildId, the binary's name).

  Fixed by normalizing them in the comparison (spec/harness.md).
- **(c) 63 `expect: ir` tests were skipped without an oracle.** The
  differential compares kiln with kiln, so it needs no oracle. They now
  run.
- **(c) 16 replays: `the command line differs`.** Only the consumer
  got the harness's report options (`-fconstexpr-report=`,
  `-fkiln-instantiation-report=json`). Under `--pch-diff` no report
  options are added. All 16 restore.
- **(c) run 3: `contracts-many.cpp` killed under its 256 MiB cap**
  with the header as an `-include`; the two stack-guard tests (fatal
  error depth). Skipped with the synthetic prefix, as above.
- **(c) run 5: `contracts-many.cpp` killed again, this time in the PCH
  build.** Both builds sit at the edge of the cap (256 MiB plus the
  1024 MiB stack reservation, 1280 MiB of address space). Measured with
  `ulimit -v`, the restore needs about 1296 MiB and the `-include` build
  1342 MiB, the same with `KILN_PCH_SNAPSHOT_PASSES=2`. So this is not
  a restore or single-pass producer problem, and which build is killed
  varies. A PCH build killed under the cap is now a skip with the
  synthetic prefix as well (spec/harness.md). The two-pass whole-corpus
  sweep was not needed: run 5 has no failure or new replay otherwise.
- **Allowed: `module.private.frag/private-without-module.cpp`** with
  the synthetic prefix replays, because it is a module unit (above).

No (b) was found.

## libc++

`cargo xtask libcxx --compiler <harness kiln++> --no-fail -j 4
[--pch-diff snapshot]` over std/utilities, std/containers,
std/algorithms, std/ranges, std/strings and std/language.support of
llvmorg-19.1.7 (4299 tests), the baseline first, then the snapshot run,
in the same session with the same binary. The pair was run on both
trees.

| run | tests | pass | fail | xfail | unsupported | with a PCH | restored | replayed | wall |
|---|---|---|---|---|---|---|---|---|---|
| no PCH, shipping tree | 4299 | 4261 | 1 | 1 | 36 | - | - | - | 2249 s |
| `--pch-diff snapshot`, shipping tree | 4299 | 4261 | 1 | 1 | 36 | 3894 | 3894 | 0 | 2329 s |
| no PCH, before WP-1393 | 4299 | 4261 | 1 | 1 | 36 | - | - | - | 861 s |
| `--pch-diff snapshot`, before WP-1393 | 4299 | 4261 | 1 | 1 | 36 | 3894 | 3894 | 0 | 1881 s |

- **Differences: none.** Every test has the same outcome in both runs
  of each pair, and the two pairs agree.
  The one FAIL is the same in both and is not about PCHs
  (`unique.ptr.special/io.verify.cpp`: an expected "invalid operands to
  binary expression" is not seen).
- **Tests without a PCH (369).** 332 are `verify`, `fail` or `sh`
  tests, which the mode compiles without a PCH (spec/libcxx.md); 37
  pass tests have no leading `#include` or select their own
  `-include`/`-x`.
- **Time.** The snapshot run builds one PCH per test (the prefix is the
  test's own `#include`s), so its wall time (the sum of the tests'
  times goes from 3209 s to 5390 s before WP-1393, and from 4587 s to
  6901 s on the shipping tree) measures the producer, not the consumer.
  The shipping-tree pair ran on a host about twice as loaded (load
  average 25 to 40), so its wall times compare with neither each other
  closely nor the first pair. It is not a speed result.

Scratch directories of both runs were removed after the run.

## CI lane

`cargo xtask ci` runs `kiln --pch-diff snapshot
--pch-diff-require-restore --pch-diff-extra-dir` (WP-1403) over the slice `cpp.include` +
`module.interface`, next to the image and replay lanes. A consumer that
replays for a reason not on the allow-list fails CI.

Cost, from the merged branch's `cargo xtask ci` (debug kiln, 8 jobs):

- snapshot lane: 34.3 s (227 tests: 89 compared, all restored; 138
  skipped);
- the image lane (`load`): 29.4 s; the replay lane: 23.1 s.

The snapshot lane is the slowest of the three: each test builds its
own PCH with `-fkiln-pch-snapshot=write`, whose producer runs a second
job for the snapshot (spec/snap.md) besides the image.

## Search path

WP-1403, 2026-11-13. A consumer whose search list differs from the
producer's repeats the prefix's recorded lookups (spec/driver.md
"Lookup-tolerant search path"). Measured with a release kiln on this
host's workspace file system (virtiofs, where one directory read takes
about 1.4 ms and a failed `stat` about 0.6 ms), each consumer adding
empty `-I` directories before the producer's, three runs each,
`snapshot-timing: find ... lookups repeated`:

| prefix | lookups | files | +1 dir | +5 dirs | +20 dirs |
|---|---|---|---|---|---|
| `<bits/stdc++.h>` | 517 | 508 | 1.1-1.7 ms | 2.5-3.0 ms | 7.1-13.2 ms |
| `<bits/stdc++.h>` + 300 headers in 2 `-I` dirs | 817 | 808 | 1.6-2.2 ms | 2.4-3.7 ms | 5.6-6.2 ms |

The 1 and 5 directory rows are the target case (under 5 ms) and meet
it. The 20 directory row is above 5 ms on this file system; it is
recorded as measured and accepted.

Equal lists take the fast path (nothing repeated, 0 ms). The cost is one
read of each directory the producer did not have, read together on up
to 8 threads, plus a hash probe per lookup and directory; a directory
the producer already searched is not asked again. With 20 directories
the reads themselves (three rounds of 1.4 ms here) are most of it; on
a local disk a directory read is tens of microseconds. For scale, the
whole `-fsyntax-only` compile of an empty main file with the
`<bits/stdc++.h>` PCH and one extra directory takes 0.51 s restored and
1.73 s replayed here.

The lookup log adds 18.3 KB to the `<bits/stdc++.h>` PCH (517 lookups,
13 directory names; 0.05% of the 36.9 MB section) and 25.7 KB to the
817-lookup prefix.

The differential: `cargo xtask test --pch-diff snapshot
--pch-diff-require-restore --pch-diff-extra-dir` over the CI slice
restores all 89 compared tests (about half with the extra directory),
and `scripts/pch-snapshot-diff.sh` has three search-path cases:
`search-extra` (restores), `search-shadow` and `search-has-include`
(refused with the expected warning; the replay equals the plain
compile).

## Default path (WP-1404)

2026-11-13, the WP-1404 branch (kiln built from `a61c44269`, release), with
`scripts/pch-release-gate.sh`, which the release gate now runs
(docs/VALIDATION.md "Release candidates"). Producer and consumer have no
snapshot option: the producer is the default build (single pass where
it may, no `-v`), the consumer the default `-include-pch`.

Whole corpus, `cargo xtask test --compiler <kiln++> --pch-diff snapshot
--pch-diff-require-restore --pch-diff-synthetic` (7071 tests, 8 jobs,
4367 s, 152 GB free at the start, scratch removed by the harness):

| pass | fail | skip | restored | replayed | failed before the consumer |
|---|---|---|---|---|---|
| 5707 | 0 | 1364 | 5706 | 1 (allowed: the module unit) | 0 |

The producers, by shape (`pch-diff snapshot producers:`; the synthetic
prefix's PCH is built once per flag set and shared, so there are fewer
producers than consumers):

| producers | single pass | two passes |
|---|---|---|
| 1419 | 1409 | 10 (all: the prefix wrote diagnostics) |

No test needed a C header or another two-pass reason here; the forced
shapes are covered by `scripts/pch-snapshot-diff.sh` (`c17`, `gnu23`: a
C header; `diag`, `diag-once`: diagnostics; `open-diag-push`: a line
read differently in a main file; `mid-two-pass`:
`KILN_PCH_SNAPSHOT_PASSES=2`), whose 20 cases all restore or replay as
expected, with the shape each case's `expect-shape` names (13 single
pass, 7 two passes).

libc++ pair, `cargo xtask libcxx --compiler <kiln++> --no-fail -j 4
[--pch-diff snapshot] --results <file>` over the six directories above
(llvmorg-19.1.7):

| run | tests | pass | fail | xfail | unsupported | with a PCH | restored | replayed | wall |
|---|---|---|---|---|---|---|---|---|---|
| no PCH | 4299 | 4261 | 1 | 1 | 36 | - | - | - | 1583 s |
| `--pch-diff snapshot`, default path | 4299 | 4261 | 1 | 1 | 36 | 3894 | 3894 | 0 | 1445 s |

**Differences: none** (the script compares the outcome of every test in
the two `--results` files). The one FAIL is the same
`unique.ptr.special/io.verify.cpp` as above, in both runs.

Two projects that use `target_precompile_headers`, built with the default
path: reflect-cpp `v0.27.1` (5 PCHs, 266 consumers, 327/327 TUs, 484/484
tests, as in the 1.0 run; a sample of 8 consumers re-run with `-v` all
say `snapshot matches; restoring it`; no `-Wpch-snapshot-unused`), and
the CMake demo of `scripts/oss/selftest.sh` (PASS, and with `-Werror`
by hand: every consumer restored, no warning). Glaze `v9.1.0`, listed
with PCH, has no `target_precompile_headers` at that tag (100/102 tests,
the same 2 TUs failing as under clang).

## 1.2.0 release candidate

2026-11-14, the 1.2.0 release candidate `2c2e2d1bb` (the release
archive's `kiln++`), `scripts/pch-release-gate.sh <kiln++> <dir> all`,
as part of the release gate ([release-1.2.0.md](release-1.2.0.md)).
Verdict of the script: pass.

Whole corpus (7087 tests, 2920 s):

| pass | fail | skip | restored | replayed | failed before the consumer |
|---|---|---|---|---|---|
| 5720 | 0 | 1367 | 5719 | 1 (allowed: the module unit) | 0 |

| producers | single pass | two passes |
|---|---|---|
| 1422 | 1412 | 10 |

libc++ pair: 4299 tests in each run (4261 pass, 1 fail, 1 xfail, 36
unsupported), 3894 compiled with a PCH in the snapshot run, no
difference in any test's outcome. The one FAIL is the same
`unique.ptr.special/io.verify.cpp` in both runs.

## Remaining differences

- The stack guard fires a few frames earlier or later after a restore
  (the 2 stack-guard tests). The diagnostic is the same fatal error at
  the same place; the depth and therefore the context lines differ.
  It is not compared.
- libc++: none.
