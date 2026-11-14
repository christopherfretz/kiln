// x86-64 psABI §3.2.3 (AVX extension), clang `X86_64ABIInfo` at the
// translation unit's AVX ABI level (`X86AVXABILevel`, from its target
// features): `-mno-avx` after `-march=haswell` turns `avx` and everything that needs it off: back to no AVX. The shapes (bare vectors, wrappers, mixed aggregates,
// varargs and `va_arg`, members, lambdas, thunks, indirect calls) are in
// Inputs/avx-abi-shapes.h. WP-1418.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -march=haswell -mno-avx -Wno-psabi -ftrivial-auto-var-init=uninitialized
#include "Inputs/avx-abi-shapes.h"
