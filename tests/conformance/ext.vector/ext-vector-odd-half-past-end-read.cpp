// [expr.const]/10.9: the lane `.hi`/`.odd` selects past the end of an
// odd-sized ext_vector_type holds no value, so an operation that reads it
// (arithmetic, narrowing to it, reading it from an object) is not a
// constant expression
//@ skip(clang): clang 22 reads such a lane as an arbitrary value, and crashes negating it
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:15 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 22:19 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 23:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 23:19 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 24:15 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 24:19 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 25:15 error "constexpr variable 'n' must be initialized by a constant expression"
//@ diag: 25:20 note "read of uninitialized object is not allowed in a constant expression"
typedef int e2i __attribute__((ext_vector_type(2)));
typedef int e3i __attribute__((ext_vector_type(3)));
constexpr e3i e3 = {1, 2, 3};
constexpr e2i h3 = (e3 + 0).hi;
constexpr int a = h3.x;
constexpr int b = h3.y;
constexpr e2i c = (e3 + 0).hi + 1;
constexpr int d = (e3 + 0).odd.y;
constexpr e2i n = -(e3 + 0).hi;
constexpr int f() { e2i v = (e3 + 0).hi; v.y = 5; return v.x + v.y; }
static_assert(f() == 8);
