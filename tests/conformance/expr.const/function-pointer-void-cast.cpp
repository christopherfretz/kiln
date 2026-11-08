// [expr.cast]/4, [expr.const]/10.15: a C-style cast between a pointer to
// function and `void *` performs a reinterpret_cast, so it is not a core
// constant expression, also when only its value is compared. clang 22 and
// GCC 16.1 reject it; clang 19 folds it (function-pointer-void-cast-19.cpp).
// Outside a constant expression the cast is fine (dynamic initialization).
// Compared with GCC 16.1 (spec/harness.md "GCC oracle"; WP-830).
//@ expect: compile-fail
//@ oracle: gcc
//@ requires: clang>=22
//@ strict-diags
//@ diag: 19:23 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 19:27 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 20:16 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 20:20 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 21:15 error "static assertion expression is not an integral constant expression"
//@ diag: 21:15 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
void f() {}
using F = void (*)();
constexpr const void *p = (const void *)&f;
constexpr bool b = (const void *)&f != nullptr;
static_assert((const void *)&f == (const void *)&f);
const void *rt = (const void *)&f;
