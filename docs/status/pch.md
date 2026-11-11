# Precompiled headers: the declaration image consumer (WP-1036, 2026-11-11)

This page measures what the PCH declaration image (spec/driver.md, "The
declaration image") buys today, and how much of the standard library it
carries. The short answer is nothing yet. No standard header set larger
than the C-compatibility headers is carried. Every realistic prefix is
replayed, so a kiln PCH costs about what textual inclusion costs.
Widening coverage is WP-1283/1284. The consumer, the fallback and the
differential that keeps the two paths equal are in place for that work.

## Setup

- kiln at the WP-1036 branch (the census re-run after merging #610),
  `harness` profile (`cargo build --profile
  harness -p kiln_driver`). clang 19.1.7 (Debian).
- aarch64 Linux, 24 cores. The host was shared with other builds.
- `-std=c++26` (clang: `-std=c++2c`).
- Every number is the best wall time of 3 runs from `/usr/bin/time -f
  "%e %M"`, with peak RSS in MB.
- Header sets:
  - `gnu`: `#include <bits/stdc++.h>` (libstdc++ 14).
  - `lcx`: the 106 public libc++ 19 headers, one `#include` each, with
    `-stdlib=libc++` (the set of
    [pch-image-census.md](pch-image-census.md)).
- The translation unit uses `std::vector`, `std::sort`, `std::map` and
  `std::string`, with no `#include`s of its own.
- Rows:
  - "no PCH": `-include <set>.h`.
  - "build": `-fkiln-pch-image -x c++-header <set>.h -o <set>.pch`.
  - "load": `-fkiln-pch-image=load -include-pch <set>.pch`.
  - "replay": `-fno-kiln-pch-image -include-pch <set>.pch`.
  - "check" is not timed: it compiles twice by design.

## `<bits/stdc++.h>` (libstdc++ 14)

Path: replay. The producer's `-v` says `consumers replay the header: a
type the image does not carry`. The consumer's says `is replayed: it has
no declaration image`.

- kiln, no PCH:
  - `-fsyntax-only`: 1.47 s, 85 MB
  - `-O0 -c`: 1.54 s, 92 MB
- kiln PCH build: 1.44 s, 85 MB
- kiln PCH, load (replay taken):
  - `-fsyntax-only`: 1.47 s, 86 MB
  - `-O0 -c`: 1.62 s, 89 MB
- kiln PCH, replay:
  - `-fsyntax-only`: 1.46 s, 84 MB
  - `-O0 -c`: 1.56 s, 89 MB
- clang 19, no PCH:
  - `-fsyntax-only`: 1.43 s, 232 MB
  - `-O0 -c`: 1.52 s, 236 MB
- clang 19 PCH build: 1.65 s, 272 MB
- clang 19 PCH:
  - `-fsyntax-only`: 0.40 s, 161 MB
  - `-O0 -c`: 0.46 s, 173 MB

The brief's baseline was kiln 0.95 s / 85 MB, clang 19 1.11 s / 236 MB
and clang 19 PCH 0.28 s / 163 MB. The memory matches it. Wall times are
about 1.4x the baseline for every compiler alike, which is host load,
not a change.

## All libc++ 19 headers

Path: replay, because `a member function template` is refused.

- kiln, no PCH:
  - `-fsyntax-only`: 1.27 s, 86 MB
  - `-O0 -c`: 1.39 s, 86 MB
- kiln PCH build: 1.26 s, 85 MB
- kiln PCH, load (replay taken):
  - `-fsyntax-only`: 1.27 s, 87 MB
  - `-O0 -c`: 1.42 s, 89 MB
- kiln PCH, replay:
  - `-fsyntax-only`: 1.24 s, 88 MB
  - `-O0 -c`: 1.39 s, 90 MB
- clang 19, no PCH:
  - `-fsyntax-only`: 1.24 s, 212 MB
  - `-O0 -c`: 1.30 s, 216 MB
- clang 19 PCH build: 1.54 s, 258 MB
- clang 19 PCH:
  - `-fsyntax-only`: 0.19 s, 145 MB
  - `-O0 -c`: 0.25 s, 155 MB

A kiln consumer that replays costs the same as no PCH, within noise.
The record is the only part read, and replay parses the header in full.
clang's PCH consumer is 5 to 6 times faster than its own no-PCH build.
That is the gap the image has to close once it carries these headers.

## The largest carried set (census)

Each header was built alone as a PCH under `-fkiln-pch-image -v`, and
the producer note was recorded. The headers that are carried were then
joined into one PCH.

- **libc++ 19 (106 headers): 8 carried.**
  - The set is `<cassert>`, `<cerrno>`, `<cfloat>`, `<ciso646>`,
    `<climits>`, `<cstdbool>`, `<cstdint>` and `<version>`.
  - Their union is carried: one image of 162 records.
  - The first refusals for the other 98, after WP-1283 (#610, which
    carries class template specializations):
    - a type: 48
    - a template argument: 20
    - a member function template: 13
    - a specialization of a template outside the image: 7
    - a default template argument: 2
    - a class member outside its class: 2 (`<cctype>`, `<cwctype>`)
    - one each: an internal image inconsistency (`<cfenv>`), an
      attribute (`<csignal>`), a parameter with an adjusted type
      (`<csetjmp>`), a type declared outside the image, a member
      template, and side data.
  - Before #610, 40 of the 98 were first refused for an explicit or
    partial specialization, and 17 for a type. The carried set is the
    same either way.
  - Timed with a TU that uses `INT_MAX` and `errno`. kiln no PCH, load
    and replay all take 0.02 s for `-fsyntax-only` (11 to 12 MB) and
    0.03 s for `-O0 -c` (52 MB). clang 19 PCH takes 0.01 s at 85 MB.
    The set is too small for the image to show a gain.
- **libstdc++ 14 (109 headers): 0 carried.**
  - 73 headers are refused for `a block-scope extern or function
    declaration`. `<bits/c++config.h>` defines `std::__terminate()`,
    which declares `void terminate()` in its body. A consumer's
    `namespace std { void terminate() noexcept; }` would redeclare it
    ([basic.link]/6). The image does not carry that link, so these
    headers are refused unless the header also declares
    `std::terminate` at namespace scope, as `<exception>` does.
  - The other 36 headers are refused for `a type the image does not
    carry`.

Re-run with a small loop: for each header `h`, write `#include <h>` to a
file `f.h` and run
`kiln++ -std=c++26 [-stdlib=libc++] -fkiln-pch-image -v -x c++-header
f.h -o f.pch`. Collect each `note: precompiled header` line. The
reasons match `-fkiln-bmi-image-census`'s first reason, except for the
sema-state refusals of spec/driver.md, which the census does not report.

## Which path the differential takes

From `cargo xtask test --pch-diff load -v` and `cargo xtask libcxx
--pch-diff load` (spec/harness.md "PCH differential").

- Conformance PCH and modules directories, `tests/differential` and
  `tests/header-guard`:
  - 141 compared, all equal in `load`, `replay` and `check`.
  - 17 took the image, 124 replayed. The replay reasons follow the
    census above.
- libc++ M3 gate (`--gate m3`):
  - 636 pass + 6 unsupported, both `load` and `replay`, as without a
    PCH.
  - 575 tests had a PCH-able prefix and every one replayed, because
    every libc++ header in a prefix is refused (see the census).

## What turns the image on for real

1. **Coverage.** The image must carry the libc++ and libstdc++ headers
   (WP-1283/1284).
2. **The block-scope declaration link.** This is needed for libstdc++.
   One fix is for the image to carry block-scope `extern`/function
   declarations in `local_externs` order. The other is to stop refusing
   them once namespace-scope matching is by namespace, not by namespace
   block.
   - A related pre-existing gap: kiln links a block-scope declaration
     only to a later redeclaration in the same namespace block.
     `namespace s { inline void t() { int k(); } } namespace s { void
     k(); }` is accepted, and clang rejects it.
3. **Make it the default.** The image becomes the default
   (`-fkiln-pch-image` on the producer, `load` on the consumer) once
   (1) is green under `--pch-diff check`.
