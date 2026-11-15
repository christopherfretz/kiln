# AArch64 NEON builtins (WP-1421, 2026-11-14)

Which function-like intrinsics of `arm_neon.h`, `arm_fp16.h` and
`arm_acle.h` kiln compiles, measured by one call of each. The lowering
rules are in spec/codegen.md, "AArch64 NEON builtins".

## How it is produced

```
source scripts/env.sh
python3 scripts/neon-intrinsic-census.py --kiln <kiln++> \
    --scratch <dir> --out <table.tsv> -j 6
```

The script generates one wrapper TU per intrinsic name, compiles it with
clang 22 (`-S -emit-llvm`) and with kiln (`-O0 -c`), both at
`--target=aarch64-unknown-linux-gnu -std=c++26`, and classifies the
result as `ok`, `unsupported` (kiln stops at "code generation for B is
not supported yet"), `error` (kiln fails otherwise) or `clang-rejects`.
Its docstring has the details. A run takes about 25 minutes at `-j 6`.

## Summary

| run | intrinsics | ok | unsupported | error | clang-rejects |
|---|---|---|---|---|---|
| before WP-1421 (main 8f93304ee) | 4884 | 3094 | 1332 | 323 | 135 |
| after WP-1421 | 4884 | 4426 | 0 | 323 | 135 |

Before WP-1421, 628 distinct builtins were unsupported. They are listed
here by lowering shape, with the number of intrinsics each group blocked.
WP-1421 implements all of them:

| group | builtins | intrinsics |
|---|---|---|
| load (dup / lane / multi-register) | 30 | 264 |
| vector arithmetic (other) | 71 | 183 |
| store (lane / multi-register) | 18 | 174 |
| saturating scalar | 97 | 125 |
| compare against zero | 41 | 103 |
| conversion (rounding / scalar) | 78 | 79 |
| scalar arithmetic | 64 | 70 |
| saturating vector | 9 | 68 |
| fixed-point conversion (`_n`) | 40 | 44 |
| lane forms | 26 | 40 |
| absolute compare | 28 | 36 |
| rounding | 32 | 36 |
| SHA3 / SHA512 / SM crypto | 31 | 31 |
| load / store (other: `bf16_x`, `vldap1`, `vstl1`) | 28 | 28 |
| compare (scalar) | 27 | 27 |
| fp16 fmlal/fmlsl | 8 | 24 |

To check the lowering, the 1332 previously unsupported wrappers were
compiled to IR by kiln and by clang++-19 (with `KILN_LLVM_DIR` set to
LLVM 19, so both use the same `opt`). All 1332 give the same IR.

Tests: `tests/conformance/target.neon/ir-*-19.cpp` (IR per group, against
clang 19), `ir-image-codec-forms-19.cpp` (six forms found building
open-source image codecs, verbatim), and the run tests
`image-codec-forms-run.cpp` and `sisd-compare-cvt-run.cpp`, which check
results against a scalar reference.

## Errors: names kiln's headers lack

The `error` rows are not codegen gaps. kiln's `resource/include` is
clang 19.1.7's headers (`scripts/gen-resource.sh`), and that copy is used
in both compat modes. The census probes the names of clang 22's headers
too, and 323 of them are not in clang 19's:

- 208 "unknown type name": the FP8 types `mfloat8x16_t` (81),
  `mfloat8_t` (58), `mfloat8x8_t` (57), the `mfloat8x*x{2,3,4}_t` tuples
  (9) and `fpm_t` (3).
- 115 "use of undeclared identifier": the `mf8` `vreinterpret*` forms
  (29), the lane forms of `vluti2`/`vluti4` (48), `vscale`, `vamin` and
  `vamax` (15), the scalar conversions with a different source and result
  width (such as `vcvtms_s64_f32`), `vcreate` forms, `__pld_range` and
  `__arm_mops_memset_tag`.

Follow-ups:

- Ship clang 22's intrinsic headers for `-fclang-compat=22`
  (`scripts/gen-resource.sh` copying one set per compat version). That
  removes the 323 errors and turns them into codegen rows.
- Then: the FP8 types (`__mfp8` and its vectors) and `fpm_t` in the type
  system, and the builtins behind `vluti2`/`vluti4`, `vamin`/`vamax`,
  `vscale` and the new scalar conversions.
