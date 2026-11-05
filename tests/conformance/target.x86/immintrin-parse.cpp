// [driver] kiln's resource headers are clang 19's x86 intrinsic headers.
// <immintrin.h>, <x86intrin.h> and <emmintrin.h> define every intrinsic
// as an inline function (fenced by `target("...")` only) whose body calls
// `__builtin_ia32_*` builtins, from `__builtin_ia32_clui` in
// <uintrintrin.h> on; clang declares every x86 builtin on x86-64 whatever
// the target features (spec/landed/sema/WP-630.md "Target builtins").
// BDE's bdlc_flathashtable_groupcontrol.h includes the first and last.
// WP-630.
//@ args: --target=x86_64-unknown-linux-gnu -std=gnu++23 -Wall -Werror
//@ std: c++23
//@ skip(clang): clang on an aarch64 host has no x86-64 glibc headers for the pinned target
//@ strict-diags
#include <immintrin.h>
#include <x86intrin.h>
#include <emmintrin.h>
