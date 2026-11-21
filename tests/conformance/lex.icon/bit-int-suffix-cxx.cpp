// clang's C++ spelling of C23's bit-precise integer suffix: `__wb` and
// `__uwb` (the `u` in either case, then `wb` or `WB`; also `u__wb` and
// `__wbu`) give the narrowest `_BitInt` holding the value, in expressions
// and in `#if`, with clang's pedantic warning (`-Wbit-int-extension`).
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wbit-int-extension
//@ diag: 18:22 warning "'_BitInt' suffix for literals is a Clang extension"
//@ diag: 19:16 warning "'_BitInt' suffix for literals is a Clang extension"
//@ diag: 19:24 warning "'_BitInt' suffix for literals is a Clang extension"
//@ diag: 20:16 warning "'_BitInt' suffix for literals is a Clang extension"
//@ diag: 20:25 warning "'_BitInt' suffix for literals is a Clang extension"
//@ diag: 21:22 warning "'_BitInt' suffix for literals is a Clang extension"
//@ diag: 22:15 warning "'_BitInt' suffix for literals is a Clang extension"
//@ diag: 22:25 warning "'_BitInt' suffix for literals is a Clang extension"
//@ diag: 23:5 warning "'_BitInt' suffix for literals is a Clang extension"
//@ diag: 26:11 error "invalid suffix '__wb1' on integer constant"
static_assert(sizeof(0x7f__wb) == 1);
static_assert((0__wb - 1__wb) < 0);
static_assert((0__uwb - 1__UWB) > 0);
static_assert(sizeof(0x1ffffffffffffffff__uwb) == 16);
static_assert(3u__wb == 3__wbu);
#if 2__uWB != 2
#error "wrong value"
#endif
auto e = 3__wb1;
