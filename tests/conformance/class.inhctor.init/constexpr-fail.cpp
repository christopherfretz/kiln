// [class.inhctor.init]/1, [dcl.constexpr]/3, [expr.const]/10.3: an
// inheriting constructor whose inherited constructor is not constexpr is
// not constexpr; calling it in a constant expression is diagnosed as the
// inherited constructor (clang `note_constexpr_invalid_function`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:15 error "static assertion expression is not an integral constant expression"
//@ diag: 13:23 note "non-constexpr constructor 'B' cannot be used in a constant expression"
//@ diag: 14:15 note "in call to 'f()'"
//@ diag: 11:12 note "declared here"
struct B { B(int i) : v(i) {} constexpr B() : v(0) {} int v; };
struct D : B { using B::B; };
constexpr int f() { D d(3); return d.v; }
static_assert(f() == 3);
