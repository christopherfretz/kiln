// x86-64 psABI §3.2.3 (AVX extension), clang `X86_64ABIInfo` at the
// translation unit's AVX ABI level (`X86AVXABILevel`, from its target
// features): `-march=x86-64-v4`: AVX-512 level. The shapes (bare vectors, wrappers, mixed aggregates,
// varargs and `va_arg`, members, lambdas, thunks, indirect calls) are in
// Inputs/avx-abi-shapes.h. WP-1418.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -march=x86-64-v4 -Wno-psabi -ftrivial-auto-var-init=uninitialized
#include "Inputs/avx-abi-shapes.h"
