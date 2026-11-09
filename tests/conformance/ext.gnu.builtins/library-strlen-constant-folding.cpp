// A call of a C library function clang's evaluator knows (`strlen`,
// `memcmp`, `strchr`, `memcpy`, ...) is evaluated as its `__builtin_` twin
// after a note that keeps it from being a constant expression: a constant
// expression fails with "non-constexpr function 'strlen'" (and no
// "declared here"), while a fold (`__builtin_constant_p`, an array bound
// folded as a GNU extension) succeeds (clang `VisitBuiltinCallExpr`,
// `CCEDiag(note_constexpr_invalid_function)`; WP-940).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 20:16 error "constexpr variable 'x' must be initialized by a constant expression"
//@ diag: 20:20 note "non-constexpr function 'strlen' cannot be used in a constant expression"
//@ diag: 21:16 error "constexpr variable 'y' must be initialized by a constant expression"
//@ diag: 21:20 note "non-constexpr function 'memcmp' cannot be used in a constant expression"
//@ diag: 24:10 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 24:10 note "non-constexpr function 'strlen' cannot be used in a constant expression"
//@ diag: 24:6 warning "variable length array folded to constant array as an extension"
extern "C" __SIZE_TYPE__ strlen(const char *);
extern "C" int memcmp(const void *, const void *, __SIZE_TYPE__);
constexpr auto x = strlen("a");
constexpr auto y = memcmp("a", "b", 1);
static_assert(__builtin_constant_p(strlen("ab")));
static_assert(__builtin_constant_p(memcmp("a", "b", 1)));
char arr[strlen("abc")];
static_assert(sizeof(arr) == 3);
