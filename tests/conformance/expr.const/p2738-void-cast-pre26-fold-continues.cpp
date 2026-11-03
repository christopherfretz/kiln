// [expr.const]/5.15 (C++11 to C++23): clang reports the pre-C++26 cast
// from `void *` as a note and keeps evaluating with the cast's result
// designating nothing (`Designator.setInvalid()`): where only a fold is
// needed (a file-scope array bound, `__builtin_constant_p`, a static
// initializer), comparing or testing the pointer still folds, while
// reading through it fails (`a2`). kiln's fold makes the result a
// pointer that points at nothing; a required constant expression stops
// at the cast with the same note (spec/oven.md "Pointers that point at
// nothing"; WP-415).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:8 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 21:8 note "cast from 'const void *' is not allowed in a constant expression in C++ standards before C++2c"
//@ diag: 21:5 warning "variable length array folded to constant array as an extension"
//@ diag: 24:8 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 24:9 note "cast from 'const void *' is not allowed in a constant expression in C++ standards before C++2c"
//@ diag: 24:5 error "variable length array declaration not allowed at file scope"
constexpr int n = 4;
constexpr const void *vp = &n;
int a1[(const int *)vp ? 1 : 2];
static_assert(sizeof(a1) == sizeof(int));
static_assert(__builtin_constant_p((const int *)vp == &n));
int a2[*(const int *)vp];
