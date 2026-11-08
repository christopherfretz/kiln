# Header search and file loading: system calls per TU (WP-820)

File-system system calls, wall time, sys time and peak RSS of kiln++
against clang++-19 for three TUs. The change itself is described in
spec/landed/pp/WP-820.md and spec/pp.md "Lookup caching (WP-820)".

Host: aarch64 Debian 13 container, workspace on a bind mount
(`/Volumes/AgentWork`), where every file-system call blocks. The load
average was about 24 during all runs, so wall times vary by tens of
percent between runs. System-call counts are exact and repeat.

## Inputs

- **fmt 12.1.0** (`scripts/oss/oss-build.sh fmt`; checkout at
  `.oss/src/fmt`, commit 407c905e4), TU `test/format-test.cc`.
- **libc++ TU**: this file, saved as `libcxx_tu.cpp`.

  ```cpp
  #include <algorithm>
  #include <ranges>
  #include <vector>
  #include <string>
  #include <map>
  int main(){ std::vector<int> v{3,1,2}; std::ranges::sort(v); auto r = v | std::views::transform([](int x){return x*2;}); return (int)std::ranges::distance(r); }
  ```

- **157 `-I` synthetic tree**, shaped like modular Boost (the WP-824
  case, Boost.Graph `libs/graph/test/lvalue_pmap.cpp` with 157 `-I`
  directories): `python3 scripts/perf/gen-include-tree.py syn157 157`
  writes `syn157/main.cpp`, 3926 headers under `syn157/libs/`, and
  `syn157/flags.txt` (157 absolute `-I` options). The generator is
  seeded, so the tree is identical on every run.

## Commands

`K` is a harness-profile kiln++ (`cargo build --profile harness -p
kiln_driver`, binary `$CARGO_TARGET_DIR/harness/kiln`), `R` is
`-resource-dir <checkout>/resource` and `F` is `.oss/src/fmt`. Each
command was run with `-E` and with `-fsyntax-only`.

```sh
# fmt
$K $R -std=c++20 -I$F/include -I$F/test/gtest -I$F/test -E $F/test/format-test.cc
clang++-19 -std=c++20 -I$F/include -I$F/test/gtest -I$F/test -E $F/test/format-test.cc
# libc++
$K $R -std=c++26 -stdlib=libc++ -E libcxx_tu.cpp
clang++-19 -std=c++26 -stdlib=libc++ -E libcxx_tu.cpp
# 157 -I synthetic tree
$K $R -std=c++20 $(cat syn157/flags.txt) -E syn157/main.cpp
clang++-19 -std=c++20 $(cat syn157/flags.txt) -E syn157/main.cpp
```

Counting (`strace` from `apt-get install strace`) and timing, each as a
separate run of the same command:

```sh
strace -f -c -o counts.txt -e trace=newfstatat,statx,fstat,openat,readlinkat,getdents64,close,faccessat,faccessat2,read,pread64,lseek <command> > /dev/null
/usr/bin/time -f "%e %U %S %M" <command> > /dev/null
```

"meta" is the sum of `newfstatat`, `statx`, `fstat`, `openat`,
`readlinkat`, `getdents64`, `faccessat` and `faccessat2`; "io" is
`read`, `pread64` and `lseek`. `close` is not counted in either.

## Results

Each cell is meta / io calls, then wall s / sys s / peak RSS KiB.
"Before" is main 26acbae10; "after" is the WP-820 branch. The "after"
and clang-19 columns come from one run; the clang values of the
"before" run are in parentheses where they differ noticeably.

### `-E`

| TU | kiln before | kiln after | clang-19 |
|---|---|---|---|
| fmt | 7363 / 873, 0.96 / 0.09 / 29804 | 1031 / 440, 0.30 / 0.04 / 30404 | 2363 / 359, 0.44 / 0.03 / 92184 (0.33 s) |
| libc++ | 9184 / 1446, 0.20 / 0.02 / 21732 | 1547 / 727, 0.17 / 0.02 / 22668 | 2439 / 695, 0.10 / 0.01 / 88824 |
| syn157 | 197219 / 7012, 61.42 / 5.63 / 33812 | 8861 / 3510, 6.27 / 0.33 / 33512 | 32034 / 3522, 18.50 / 0.97 / 105084 (13.60 s) |

### `-fsyntax-only`

| TU | kiln before | kiln after | clang-19 |
|---|---|---|---|
| fmt | 7373 / 873, 3.59 / 0.13 / 109100 | 1041 / 440, 2.42 / 0.06 / 112196 | 2361 / 358, 2.85 / 0.13 / 286076 (2.45 s) |
| libc++ | 9194 / 1446, 0.86 / 0.04 / 47508 | 1557 / 727, 0.68 / 0.03 / 47888 | 2437 / 694, 0.68 / 0.03 / 166756 (0.58 s) |
| syn157 | 197229 / 7012, 62.61 / 4.79 / 36100 | 8871 / 3510, 6.50 / 0.21 / 35612 | 32032 / 3521, 14.76 / 0.78 / 112456 (13.58 s) |

The `-E` output of each TU is byte-identical before and after.

## Where the calls went

Before, by kind (`-E`):

- fmt: 3249 `statx`, 3435 `readlinkat`, 223 `faccessat2`.
- libc++: 6090 of 9184 were `readlinkat`, from `fs::canonicalize` on
  every opened file (one per path component of
  `.toolchain/llvm-project/...`).
- syn157: 141,354 `statx` (an uncached `metadata` probe of every
  candidate in every one of 157 directories) and 52,336 `readlinkat`.

After, kiln makes one `statx` per distinct directory (cached), one
`openat` + `fstat` + `read` per header found, an `openat` per miss until
a directory has missed four times, and then one listing (`getdents64`)
of that directory. On syn157: 3987 `openat`, 4397 `statx`, 314
`getdents64`, 3510 `read`.

clang makes fewer `statx` but more `openat` (it opens each candidate,
14,007 on syn157) and `readlinkat` (443 on fmt, 728 on libc++).

## Not measured

The real Boost 1.90 Boost.Graph TU (WP-812 measured kiln `-E` at 67.0 s
wall against clang++-19's 17.9 s) was not re-run; the synthetic tree has
its shape (157 `-I` directories, a few thousand headers, include guards
and `#pragma once`).
