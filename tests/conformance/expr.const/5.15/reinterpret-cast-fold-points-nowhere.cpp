// [expr.const]/5.15: a reinterpret_cast or an integer-to-pointer cast is
// not a core constant expression, but clang's fold (a file-scope array
// bound, `__builtin_constant_p`) notes the cast and continues with a
// pointer that points at nothing: testing, comparing (by address) and
// subtracting it fold, reading through it does not (`i`), and an integer
// cast to a pointer and back keeps its value (`f`). A same-type
// reinterpret_cast keeps the pointer valid (`e`). Same output at
// -fclang-compat=19 (spec/oven.md "Pointers that point at nothing").
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 39:7 note "reinterpret_cast is not allowed in a constant expression"
//@ diag: 39:5 warning "variable length array folded to constant array as an extension"
//@ diag: 40:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 40:7 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 40:5 warning "variable length array folded to constant array as an extension"
//@ diag: 41:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 41:7 note "reinterpret_cast is not allowed in a constant expression"
//@ diag: 41:5 warning "variable length array folded to constant array as an extension"
//@ diag: 42:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 42:8 note "reinterpret_cast is not allowed in a constant expression"
//@ diag: 42:5 warning "variable length array folded to constant array as an extension"
//@ diag: 43:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 43:7 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 43:5 warning "variable length array folded to constant array as an extension"
//@ diag: 44:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 44:7 note "reinterpret_cast is not allowed in a constant expression"
//@ diag: 44:5 warning "variable length array folded to constant array as an extension"
//@ diag: 45:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 45:7 note "reinterpret_cast is not allowed in a constant expression"
//@ diag: 45:5 warning "variable length array folded to constant array as an extension"
//@ diag: 46:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 46:8 note "reinterpret_cast is not allowed in a constant expression"
//@ diag: 46:5 error "variable length array declaration not allowed at file scope"
constexpr int n = 4;
struct S { int a, b; };
constexpr S s{1, 2};
int b[reinterpret_cast<const char *>(&n) ? 1 : 2];
int c[(int *)4 ? 1 : 2];
int d[reinterpret_cast<const int *>(reinterpret_cast<const char *>(&n)) == &n ? 1 : 2];
int e[*reinterpret_cast<const int *>(&n)];
int f[(long)(int *)8];
int g[reinterpret_cast<const char *>(&n) + 1 != nullptr ? 1 : 2];
int h[reinterpret_cast<const int *>(&s) + 1 == &s.b ? 1 : 2];
int i[*reinterpret_cast<const char *>(&n)];
static_assert(sizeof(f) == 8 * sizeof(int));
static_assert(__builtin_constant_p((int *)8));
static_assert(__builtin_constant_p((int *)8 + 1 == (int *)12));
static_assert(__builtin_constant_p(reinterpret_cast<long>((int *)8)));
static_assert(__builtin_constant_p(reinterpret_cast<const char *>(&n) + 2 - reinterpret_cast<const char *>(&n)));
static_assert(__builtin_constant_p(reinterpret_cast<const char *>(&n) < reinterpret_cast<const char *>(&n) + 1));
static_assert(__builtin_constant_p(reinterpret_cast<const int *>(&s) != &n));
static_assert(!__builtin_constant_p(reinterpret_cast<const char *>(&n)));
static_assert(!__builtin_constant_p(*reinterpret_cast<const char *>(&n)));
static_assert(!__builtin_constant_p((int *)8 == &n));
static_assert(!__builtin_constant_p((long)(const int *)&n));
