//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "constexpr variable 'i' must be initialized by a constant expression"
//@ diag: 15:19 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 16:15 error "constexpr variable 'k' must be initialized by a constant expression"
//@ diag: 16:19 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 17:15 error "constexpr variable 'r' must be initialized by a constant expression"
//@ diag: 17:19 note "reinterpret_cast is not allowed in a constant expression"
// [expr.cast]/4.4, [expr.reinterpret.cast]/11: a cast to a reference to an
// unrelated type that no static_cast performs is a reinterpret_cast, which
// a core constant expression may not evaluate ([expr.const]/10.15), also
// in its C-style and functional forms.
constexpr float f = 1;
using IR = int &;
constexpr int i = (int &)f;
constexpr int k = IR(f); // the functional form
constexpr int r = reinterpret_cast<const int &>(f);
// A static_cast, binding to a temporary int: a constant.
constexpr int j = (const int &)f;
static_assert(j == 1);
