// x86-64 psABI §3.2.3 (AVX extension), clang `X86_64ABIInfo` at the
// translation unit's AVX ABI level (`X86AVXABILevel`, from its target
// features): `-mavx2` implies `avx`: AVX level. The shapes (bare vectors, wrappers, mixed aggregates,
// varargs and `va_arg`, members, lambdas, thunks, indirect calls) are in
// Inputs/avx-abi-shapes.h. WP-1418.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -mavx2 -Wno-psabi -ftrivial-auto-var-init=uninitialized
#include "Inputs/avx-abi-shapes.h"
