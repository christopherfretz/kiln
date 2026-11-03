//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
// From C++23 on, clang checks a constexpr body only under -Winvalid-constexpr.
//@ diag: 19:30 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 20:31 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 22:32 warning "overflow in expression; result is -2 with type 'int'"
//@ diag: 23:34 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 24:33 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 25:44 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 26:49 warning "overflow in expression; result is -2 with type 'int'"
//@ diag: 27:33 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
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
