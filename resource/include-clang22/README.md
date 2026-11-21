# kiln builtin headers: the clang 22 overlay

At `-fclang-compat=22` (the default) the driver searches this directory
before `resource/include` (spec/driver.md "Toolchain"), so these three
clang 22 headers replace their clang 19 copies there; at
`-fclang-compat=19` they are not searched.

- `arm_sve.h`, `arm_sme.h`: the clang 22 SVE and SME ACLE intrinsics,
  including the FP8 ones (`svmfloat8_t`, `fpm_t`) that clang 19 lacks.
- `arm_vector_types.h`: included by both and by `arm_neon.h`; clang 22's
  adds `mfloat8_t`, `fpm_t`, the `__ARM_FPM_*` enums and helpers and the
  `mfloat8x8_t` / `mfloat8x16_t` NEON types.

The other Arm headers (`arm_neon_sve_bridge.h`, `arm_bf16.h`) are
byte-identical between clang 19 and 22 and are not repeated here.

- **Source:** Debian package `libclang-common-22-dev` 1:22.1.8-1~deb13u5
  (`Debian clang version 22.1.8 (1~deb13u5)`), directory
  `/usr/lib/llvm-22/lib/clang/22/include`.
- **License:** Apache-2.0 WITH LLVM-exception; full text in `LICENSE.TXT`
  (the same text as `resource/include/LICENSE.TXT`).
- **kiln changes:** none; every file is byte-identical to the source.
- **Regenerate:** `scripts/gen-resource.sh` (with `CLANG22=` naming a
  clang 22 if `clang-22` is not on `PATH`).
