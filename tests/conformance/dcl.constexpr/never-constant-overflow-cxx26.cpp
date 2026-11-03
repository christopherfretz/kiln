//@ expect: compile-fail
//@ std: c++26
//@ args: -Winvalid-constexpr
//@ strict-diags
//@ diag: 39:30 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 39:15 error "constexpr function never produces a constant expression"
//@ diag: 39:30 note "value 2147483648 is outside the range of representable values of type 'int'"
//@ diag: 40:31 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 40:15 error "consteval function never produces a constant expression"
//@ diag: 40:31 note "value 2147483648 is outside the range of representable values of type 'int'"
//@ diag: 42:32 warning "overflow in expression; result is -2 with type 'int'"
//@ diag: 42:15 error "constexpr function never produces a constant expression"
//@ diag: 42:32 note "value 4294967294 is outside the range of representable values of type 'int'"
//@ diag: 43:34 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 43:16 error "constexpr function never produces a constant expression"
//@ diag: 43:34 note "value 2147483648 is outside the range of representable values of type 'int'"
//@ diag: 44:33 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 44:15 error "constexpr function never produces a constant expression"
//@ diag: 44:33 note "value 2147483648 is outside the range of representable values of type 'int'"
//@ diag: 45:44 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 45:29 error "constexpr function never produces a constant expression"
//@ diag: 45:44 note "value 2147483648 is outside the range of representable values of type 'int'"
//@ diag: 46:49 warning "overflow in expression; result is -2 with type 'int'"
//@ diag: 46:33 error "constexpr function never produces a constant expression"
//@ diag: 46:49 note "value 4294967294 is outside the range of representable values of type 'int'"
//@ diag: 47:33 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 47:16 error "constexpr function never produces a constant expression"
//@ diag: 47:33 note "value 2147483648 is outside the range of representable values of type 'int'"
//@ diag: 48:15 error "constexpr function never produces a constant expression"
//@ diag: 48:34 note "value 2147483648 is outside the range of representable values of type 'int'"
//@ diag: 49:16 error "constexpr function never produces a constant expression"
//@ diag: 49:36 note "value 2147483648 is outside the range of representable values of type 'int'"
// [dcl.constexpr]/6: a constexpr or consteval function no call of which is
// a constant expression is ill-formed, no diagnostic required. clang (and
// kiln, for these shapes) diagnoses a function without parameters whose
// body always overflows, at its name, after the body's -Winteger-overflow
// (an error by default before C++23; -Winvalid-constexpr). WP-468.
constexpr int g = 2147483647;
constexpr int f() { return g + 1; }
consteval int ce() { return g + 1; }
constexpr int fwd();
constexpr int fwd() { return g * 2; }
constexpr long wide() { return g + 1; }
constexpr int two() { int a = g + 1; return a + g + 1; }
namespace N { constexpr int neg() { return -(-g - 1); } }
struct S { static constexpr int sm() { return g * 2; } };
constexpr bool cmp() { return g + 1 > 0; }
constexpr int inc() { int i = g; ++i; return i; }
constexpr void vd() { int i = g; i += 1; }
// Not diagnosed: a failure in a callee, unsigned wrap-around, an operand
// not evaluated, a narrowing conversion, a function that can succeed.
constexpr int callee() { return f(); }
constexpr unsigned un() { return 0u - 1u; }
constexpr int cond() { return true ? 1 : g + 1; }
constexpr int sc() { short s = 32767; s += 1; return s; }
constexpr int ok() { return g - 1; }
static_assert(ok() == 2147483646);
// Templates are not checked; their specializations neither.
template <class T> constexpr T tf() { return T(g) + 1; }
