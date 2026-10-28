# kiln builtin headers (imported from clang 19)

These are clang's builtin ("resource dir") headers: `stddef.h`, `stdarg.h`,
`stdint.h`, `limits.h`, `float.h`, the `__stddef_*` / `__stdarg_*` pieces,
the Arm (`arm_*.h`) and x86 (`*intrin.h`) intrinsic headers, `module.modulemap`,
and friends. libc++ and glibc headers depend on their exact contents, so kiln
ships them unchanged (see `spec/pp.md`, `#include` resolution).

- **Source:** Debian package `libclang-common-19-dev` 1:19.1.7-3+b1
  (`Debian clang version 19.1.7 (3+b1)`), directory
  `/usr/lib/llvm-19/lib/clang/19/include` (= `clang-19 -print-resource-dir` + `/include`).
- **License:** Apache-2.0 WITH LLVM-exception; full text in `LICENSE.TXT`
  (verbatim `clang/LICENSE.TXT` at tag `llvmorg-19.1.7`).
- **kiln changes:** none. Every file is byte-identical to the source.
  Any future local change must be listed here, file by file, with the reason.
- **Regenerate:** `scripts/gen-resource.sh` (re-copies; leaves `LICENSE.TXT`
  and this README alone).

## What was skipped, and why

kiln targets C/C++ on Linux aarch64 and x86_64. Of 241 files in the source,
160 are kept (8.0 MiB of 13.5 MiB). Skipped:

| Skipped | Why |
|---|---|
| `cuda_wrappers/`, `openmp_wrappers/`, `__clang_cuda_*.h`, `__clang_hip_*.h` | CUDA / HIP / OpenMP offload; injected only by those language modes |
| `opencl-c.h`, `opencl-c-base.h` | OpenCL only |
| `llvm_libc_wrappers/` | used only when building against LLVM libc for GPU targets |
| `ppc_wrappers/`, `altivec.h`, `htmintrin.h`, `htmxlintrin.h` | PowerPC |
| `zos_wrappers/`, `s390intrin.h`, `vecintrin.h` | SystemZ / z/OS |
| `hexagon_*.h`, `hvx_hexagon_protos.h` | Hexagon |
| `larchintrin.h`, `lsxintrin.h`, `lasxintrin.h` | LoongArch |
| `msa.h` | MIPS |
| `riscv_*.h`, `sifive_vector.h` | RISC-V |
| `velintrin*.h` | NEC VE |
| `wasm_simd128.h` | WebAssembly |
| `arm_mve.h`, `arm_cde.h` | AArch32 M-profile only; both `#error` unconditionally unless `__ARM_FEATURE_MVE`/`__ARM_FEATURE_CDE`, which no aarch64 target defines (1.5 MiB) |
| `profile/`, `xray/` (and `fuzzer/`, `orc/`, `sanitizer/` if present) | compiler-rt runtime interfaces, not compiler builtins |

Kept although marginal (small, and an x86/Arm target can reach them):
the MSVC-compat headers (`intrin.h`, `intrin0.h`, `vadefs.h`, `yvals_core.h`,
`armintr.h`, `arm64intr.h`; on Linux they `#include_next` or are inert),
`arm_cmse.h`, `ptrauth.h`, `cet.h`, `builtins.h`, `ISO_Fortran_binding.h`.

## Consequences of skipping

- `__has_include(<altivec.h>)` (etc.) is true under clang on any target but
  false under kiln. Code that includes a skipped header gets "file not found"
  instead of clang's arch-specific `#error`. Both only affect programs that are
  already wrong for our targets.
- `module.modulemap` still names `altivec.h`, `s390intrin.h`, `htmintrin.h`,
  `htmxlintrin.h`, `vecintrin.h`. Each sits in a module with an unmet
  `requires` (`altivec`, `systemz`), which clang treats as unavailable rather
  than an error when its header is missing. kiln must do the same if it ever
  reads this module map.
