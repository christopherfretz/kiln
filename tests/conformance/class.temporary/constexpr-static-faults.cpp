// [expr.const]/4, /5.9, [class.temporary]/6: only a temporary of const
// non-volatile type that is lifetime-extended by a variable usable in
// constant expressions may be read outside the expression that created
// it; a non-const one is still a permitted result.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:15 note "read of temporary is not allowed in a constant expression outside the expression that created the temporary"
//@ diag: 18:19 note "temporary created here"
//@ diag: 21:15 error "static assertion expression is not an integral constant expression"
//@ diag: 21:15 note "read of temporary is not allowed in a constant expression outside the expression that created the temporary"
//@ diag: 20:21 note "temporary created here"
//@ diag: 27:15 error "constexpr variable 'k' must be initialized by a constant expression"
//@ diag: 26:28 note "read of temporary is not allowed in a constant expression outside the expression that created the temporary"
//@ diag: 27:19 note "in call to 'f()'"
//@ diag: 18:19 note "temporary created here"
struct S { int a, b; };
constexpr S &&t = S{3, 4};
static_assert(t.b == 4);
constexpr int &&i = 5;
static_assert(i == 5);
constexpr const int *pi = &i;
const int &c = 6;
constexpr const int *pc = &c;
constexpr int g = c;
constexpr int f() { return t.a; }
constexpr int k = f();
