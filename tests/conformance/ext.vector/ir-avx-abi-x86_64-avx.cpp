// x86-64 psABI §3.2.3 (AVX extension), clang `X86_64ABIInfo` at the
// translation unit's AVX ABI level (`X86AVXABILevel`, from its target
// features): `-mavx`: AVX level, 256-bit vectors and their one-member wrappers in `ymm` registers; 512-bit ones still in memory. The shapes (bare vectors, wrappers, mixed aggregates,
// varargs and `va_arg`, members, lambdas, thunks, indirect calls) are in
// Inputs/avx-abi-shapes.h. WP-1418.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -mavx -Wno-psabi -ftrivial-auto-var-init=uninitialized
#include "Inputs/avx-abi-shapes.h"
