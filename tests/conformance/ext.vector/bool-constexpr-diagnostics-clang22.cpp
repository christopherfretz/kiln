// Clang ext vectors, bool element type, [clang 20+] ([expr.const]; clang
// VectorExprEvaluator, lvalue vector element access): a lane subscript
// past the end, at the end or negative and a read of an uninitialized
// lane are not constant expressions; bool vector arguments print lane by
// lane in the call notes. WP-450.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ requires: clang>=22
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:35 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 40:15 note "in call to 'rd(8)'"
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:35 note "cannot refer to element 9 of array of 8 elements in a constant expression"
//@ diag: 41:15 note "in call to 'rd(9)'"
//@ diag: 42:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:35 note "cannot refer to element 18446744073709551615 of array of 8 elements in a constant expression"
//@ diag: 42:15 note "in call to 'rd(-1)'"
//@ diag: 43:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:43 note "assignment to dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 43:15 note "in call to 'wr(8)'"
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:43 note "cannot refer to element 10 of array of 8 elements in a constant expression"
//@ diag: 44:15 note "in call to 'wr(10)'"
//@ diag: 45:15 error "static assertion expression is not an integral constant expression"
//@ diag: 37:43 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 45:15 note "in call to 'get({true, false, true, false, false, false, false, false, false, false, false, false}, 12)'"
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 38:36 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 46:15 note "in call to 'un()'"
typedef bool b8 __attribute__((ext_vector_type(8)));
typedef bool b12 __attribute__((ext_vector_type(12)));

constexpr b8 A = {true, false, true, true, false, false, false, true};
constexpr bool rd(int i) { return A[i]; }
constexpr bool wr(int i) { b8 v = A; v[i] = true; return v[0]; }
constexpr bool get(b12 v, int i) { return v[i]; }
constexpr bool un() { b8 v; return v[1]; }

static_assert(rd(8));
static_assert(rd(9));
static_assert(rd(-1));
static_assert(wr(8));
static_assert(wr(10));
static_assert(get((b12){true, false, true}, 12));
static_assert(un());
