# kiln 1.0 release measurements

This page holds the measurements of the 1.0 release candidate. Each
section has an empty table and the exact command that fills each row.
The numbers in [../COMPATIBILITY.md](../COMPATIBILITY.md),
[../KNOWN_ISSUES.md](../KNOWN_ISSUES.md), the README and the CHANGELOG
come from the pages the sections name; when a section's run gives a
different number, update those pages in the same change.

- Release candidate: `<sha>` (fill in), version `1.0.0`.
- Host: aarch64 Linux unless a section says otherwise. Record the load
  average of each run, since the host is shared.
- Every command runs from the kiln checkout after
  `source scripts/env.sh`, which sets the target directory, the shared
  jobserver and `KILN_GCC` (GCC 16.1's `g++-16`, when the checkout has
  one). Run one heavy measurement at a time.

The commands were checked on a subset when this page was written
(WP-1202): one bench entry per compiler, one libc++ test, one corpus
directory. The full runs have not been made yet.

## RC gate: the release archive

The release candidate is the archive `cargo xtask dist` builds. Before
any measurement below, build it once, unpack it and use its `kiln++`.
This is a gate: if a step fails, there is no release candidate.

```sh
timeout 1800 cargo xtask dist --check
# writes <target>/kiln-dist/kiln-1.0.0-<triple>.tar.xz, the -symbols
# archive and SHA256SUMS; --check smoke-tests --version, the resource
# directory, hello world, a <meta> TU and a constexpr TU
D=$CARGO_TARGET_DIR/kiln-dist
(cd "$D" && sha256sum -c --ignore-missing SHA256SUMS)
mkdir -p "$D/unpacked" && tar -xJf "$D"/kiln-1.0.0-*[!s].tar.xz -C "$D/unpacked"
export PATH=$(echo "$D"/unpacked/kiln-1.0.0-*/bin):$PATH
kiln++ --version                     # kiln version 1.0.0 ...
```

Then compile and run three programs with the unpacked `kiln++`:

```sh
cat > hello.cpp <<'EOF'
#include <cstdio>
int main() { std::puts("hello"); }
EOF
kiln++ hello.cpp -o hello && ./hello                         # hello

cat > imp.cpp <<'EOF'
import std;
int main() { std::println("import std {}", std::vector{1, 2, 3}.size()); }
EOF
kiln++ -std=c++23 -stdlib=libc++ imp.cpp -o imp && ./imp      # import std 3

cat > refl.cpp <<'EOF'
#include <meta>
#include <cstdio>
struct P { int x; double y; };
int main() {
  constexpr auto n = std::meta::nonstatic_data_members_of(
      ^^P, std::meta::access_context::unchecked()).size();
  std::printf("%zu %s\n", n,
              std::meta::identifier_of(^^P).data());
}
EOF
kiln++ -std=c++26 refl.cpp -o refl && ./refl                 # 2 P
```

| step | result |
|---|---|
| `cargo xtask dist --check` | |
| `sha256sum -c` | |
| `kiln++ --version` | |
| hello world | |
| `import std;` (`-stdlib=libc++`) | |
| reflection | |

## (a) Constant evaluation: time and memory

The bench corpus's constexpr (`bench/oven`), reflection (`bench/reflect`)
and front-end (`bench/frontend`) entries, measured with the release
`kiln++` against clang 19, clang 22 and GCC 16.1. One compiler is
compared per run. Without `--record` nothing is written to the
baselines; each run prints its ratio table and writes its results file
under `bench/results/`. Copy the geometric means into the table and keep
the results files' names.

```sh
K=$(command -v kiln++)
for cc in clang++-19 clang++-22 "$KILN_GCC"; do
  cargo xtask bench --compiler "$K" --compare-with "$cc" \
      bench/oven bench/reflect bench/frontend
done
```

GCC 16.1 gets the GCC oracle's flag mapping (`-fconstexpr-steps=N` is
`-fconstexpr-ops-limit=N`, `-stdlib=` and `-fkiln-*` are dropped). It
cannot compile the `mode: emit-llvm` entries or `-ftime-trace`, and those
show as failed for GCC: count only the entries both compilers measured,
as the ratio table does.

| compared with | entries both measured | geomean time kiln/other | geomean peak RSS kiln/other | results file | load |
|---|---|---|---|---|---|
| clang++-19 (19.1.7) | | | | | |
| clang++-22 (22.1.8) | | | | | |
| g++-16 (16.1.0) | | | | | |

The README's and CHANGELOG's 0.21x / 0.17x are the clang 19 row of
[perf.md](perf.md) (2026-11-03). To make this run the new reference, add
`--record` to the clang 19 run, which also merges its table into
perf.md, and commit the baselines with it.

Validated on a subset: the same command with `--filter oven/fib
--runs 1`, once per compiler.

## (b) libc++ 22.1.8 test suite (aarch64)

The whole suite, `std` and `libcxx`, at `-std=c++26`, as
[libcxx-m5.md](libcxx-m5.md) runs it, with the release `kiln++`. It takes
about 70 minutes at `-j 3`.

```sh
cargo xtask libcxx std libcxx --tag llvmorg-22.1.8 --compiler "$(command -v kiln++)" \
    --no-fail -j 3 --timeout 20 --mem-limit 3072 \
    --report scratchpad/libcxx-rc.md --results scratchpad/libcxx-rc.tsv
python3 scripts/libcxx-results.py scratchpad/libcxx-rc.tsv > scratchpad/libcxx-rc-summary.md
```

`--report` writes the per-directory table (tests, pass, fail, xfail,
unsupported, pass rate per directory) and the failure clusters; that file
is the per-directory failure list. `libcxx-results.py` lists every
failing test with its detail and cluster key. Commit both under
`docs/status/` (for example `libcxx-1.0.md`) and link them here.

| | total | pass | fail | xfail | unsupported (driver) | timeout | pass rate | per-directory list |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| 1.0 RC | | | | | | | | |
| last full run (libcxx-m5.md) | 9247 | 8716 | 9 | 64 | 457 (31) | 1 | 99.2% | [libcxx-m5.md](libcxx-m5.md) |

The pass rate is pass / (total - unsupported), as `--report` computes
it. The 9 failures of the last run were fixed since (WP-460, WP-491);
this run confirms it.

Validated on a subset: the same command with one test path instead of
`std libcxx`, `std/utilities/utility/pairs/pairs.pair/ctor.default.pass.cpp`.

## (c) x86_64 sweep

The x86_64 numbers come from `.github/workflows/sweep.yml` on GitHub's
x86_64 runners ([../VALIDATION.md](../VALIDATION.md), "x86_64 sweep").
Start it by pushing the release candidate to a `sweep/` branch:

```sh
git push origin <rc-sha>:refs/heads/sweep/rc-1.0.0
```

How to read the run (Actions tab, the `sweep` workflow):

- `build (dist)` builds the archive once; every other job uses that
  `kiln++`. If it fails, nothing else ran.
- `libcxx summary` merges the six shards. Its job summary (and the
  `libcxx-summary` artifact) has the counts per outcome for `std/` and
  `libcxx/`, the pass rate, and every failing test. To mark the
  x86_64-only failures, download the shards' `libcxx-*.tsv` artifacts
  and run
  `python3 scripts/libcxx-results.py --baseline scratchpad/libcxx-rc.tsv <the x86_64 tsv files>`
  with the aarch64 results of (b): `new` failures are x86_64-only.
- `oss summary` (and the `oss-summary` artifact) has one table per
  project, kiln against the clang 19 control on the same runner: TUs,
  links, tests, wall time, peak TU RSS, and kiln's failure clusters. A
  project is at parity when its test counts match the control's.
- A failed `oss <project> <compiler>` job is a failed build or test run
  of that project; its `test.log` and `configure.log` artifacts say why.

| suite / project | x86_64 kiln | clang 19 control | parity |
|---|---|---|---|
| libc++ 22.1.8 (9247 tests) | | — | |
| fmt | | | |
| nlohmann_json | | | |
| abseil | | | |
| llvm_project (`SupportTests`, `ADTTests`, `IRTests`) | | | |
| duckdb | | | |
| grpc | | | |
| bitcoin_core (TU count too) | | | |
| cmake_app | | | |
| opencv (TU count too) | | | |
| daw_json_link | | | |

Record the run in [x86_64-sweep.md](x86_64-sweep.md) (run id, commit,
the table) and delete the branch afterwards:
`git push origin :refs/heads/sweep/rc-1.0.0`.

## (d) Open-source projects (aarch64)

Each project of `scripts/oss/projects.toml` is built and tested with
its own build system, once with the release `kiln++` and once with the
clang 19 control. `KILN_OSS_CXX` points the harness at the release
binary.

```sh
export KILN_OSS_CXX=$(command -v kiln++)
scripts/oss/oss-build.sh <project> --compiler clang
scripts/oss/oss-build.sh <project>
scripts/oss/oss-report.py --perf --update docs/status/oss.md <project>
```

`oss-report.py --update` rewrites the project's section of
[oss.md](oss.md) and its row of the projects table. Rebuild the control
only when the project's tag changed. Run the large projects (LLVM,
Boost, folly, gRPC) one at a time.

| project | tag | kiln tests | clang 19 tests | rating |
|---|---|---|---|---|
| (one row per project of the projects table in oss.md) | | | | |

The rows the docs depend on are the 47 libraries and applications of
[../COMPATIBILITY.md](../COMPATIBILITY.md#libraries-and-applications)
(43 Flawless, 3 Near-perfect, 1 Partial) and the two module rows,
`fmt_module` and `cmake_modules`. The re-runs owed by
[../KNOWN_ISSUES.md](../KNOWN_ISSUES.md#open-source-projects-with-gaps)
come first: simdjson and folly::coro (WP-992).

## (e) Conformance

The paper table, [../STANDARD_SUPPORT.md](../STANDARD_SUPPORT.md), and
its checker:

```sh
cargo xtask features --summary      # the counts and the summary table only
cargo xtask features --compiler "$(command -v kiln++)"   # every done row's tests
```

| standard | rows | done | partial | todo | untested |
|---|---:|---:|---:|---:|---:|
| C++98/03 | | | | | |
| C++11 | | | | | |
| C++14 | | | | | |
| C++17 | | | | | |
| C++20 | | | | | |
| C++23 | | | | | |
| C++26 | | | | | |
| C++26 reflection | | | | | |
| DR-level papers | | | | | |
| kiln extensions | | | | | |

At WP-1202 (`features --summary`): 261 rows, 260 done, 1 partial (P2786,
removed from C++26 at Kona).

The corpus counts are the last lines of the full CI run, with the clang
22 self-check:

```sh
cargo xtask ci 2>&1 | tee scratchpad/ci-rc.log
grep '^xtask ci: corpus' scratchpad/ci-rc.log
```

| corpus run | passed | failed | xfail | xpass | skipped | compared with gcc | total |
|---|---:|---:|---:|---:|---:|---:|---:|
| clang++-22 self-check | | | | | | | |
| kiln | | | | | | | |
| kiln `--bmi-kind 1` | | | | | | | |
| kiln `--bmi-kind 2` | | | | | | | |

Every `xfail` of the kiln run is listed in
[../KNOWN_ISSUES.md](../KNOWN_ISSUES.md#open-expected-failure-tests);
regenerate that list from `git grep -n 'xfail(kiln)' tests/` when the
count changes. One corpus directory can be checked alone with
`cargo xtask test --compiler "$(command -v kiln++)" tests/conformance/<dir>`.

Validated on a subset: `features --summary`, and `cargo xtask test`
on `tests/conformance/basic.indet`.

## (f) Accepts-invalid

clang 22.1.8's negative tests through kiln, as
[accepts-invalid.md](accepts-invalid.md) describes:

```sh
scripts/fetch-clang-tests.sh            # once: .oss/clang-tests, about 50 MB
cargo xtask clang-neg -j 4 --compiler "$(command -v kiln++)" \
    --json scratchpad/clang-neg.json \
    --report docs/status/accepts-invalid.md CXX SemaCXX
```

`--report` rewrites only the generated region of accepts-invalid.md; the
triage after it is by hand.

| measure | last run (`321baa9de`) | 1.0 RC |
|---|---|---|
| files where clang expects an error and kiln reports none | 7.1% (89 of 1250) | |
| `expected-error` lines with no kiln error | 17.6% (4930 of 28065) | |
| `expected-no-diagnostics` configurations kiln rejects | 11.2% (47 of 418) | |
| crashes, timeouts, out of memory | 0 of 2850 | |

Validated: `cargo xtask clang-neg --help` (the clang test checkout was
not fetched for WP-1202).

## (g) Memory safety

kiln is safe Rust: every crate of the workspace forbids `unsafe` code,
and the hermetic exception list of
[../ARCHITECTURE.md](../ARCHITECTURE.md) (principle 1) is empty. Check
both by grep:

```sh
# every crate root carries the attribute: prints nothing
for f in crates/*/src/lib.rs crates/*/src/main.rs xtask/src/main.rs; do
  [ -f "$f" ] && ! grep -q '^#!\[forbid(unsafe_code)\]' "$f" && echo "missing: $f"
done
# no unsafe block, fn or impl anywhere in kiln's code: prints nothing
git grep -nE '\bunsafe *(\{|fn|impl)' -- crates xtask
# no crate opts out
git grep -n 'allow(unsafe_code)' -- crates xtask
```

| check | result |
|---|---|
| crate roots without `#![forbid(unsafe_code)]` | |
| `unsafe` blocks, functions or impls | |
| `allow(unsafe_code)` | |
| hermetic exceptions (ARCHITECTURE.md principle 1) | none |
| third-party crates (`Cargo.lock`) | |

The third-party crates are not covered by the attribute. At WP-1202
`Cargo.lock` lists `errno`, `hashbrown`, `libc`, `signal-hook`,
`signal-hook-registry`, `windows-link` and `windows-sys` besides kiln's
own crates; list them in the table with
`grep '^name' Cargo.lock`.
