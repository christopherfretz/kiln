// [expr.const]/18-20 (WP-158): an implicit constructor's implicit
// mem-initializers are full-expressions of an immediate-escalating
// function. A base or member (with no default member initializer) whose
// default, copy or move construction calls a consteval constructor that
// is not a constant expression makes the constructor immediate ("its
// body" for a base, "the default initializer of" for a member); its call
// is then reported. A user-provided constructor reports the call itself.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:25 error "call to consteval function 'B::B' is not a constant expression"
//@ diag: 31:37 note "read of non-const variable 'g' is not allowed in a constant expression"
//@ diag: 34:25 note "in call to 'B()'"
//@ diag: 30:5 note "declared here"
//@ diag: 36:5 error "call to immediate function 'E::E' is not a constant expression"
//@ diag: 32:8 note "'E' is an immediate constructor because the default initializer of 'b' contains a call to a consteval constructor 'B' and that call is not a constant expression"
//@ diag: 31:37 note "read of non-const variable 'g' is not allowed in a constant expression"
//@ diag: 32:8 note "in call to 'B()'"
//@ diag: 36:5 note "in call to 'E()'"
//@ diag: 30:5 note "declared here"
//@ diag: 37:6 error "call to immediate function 'E2::E2' is not a constant expression"
//@ diag: 33:8 note "'E2' is an immediate constructor because its body contains a call to a consteval constructor 'B' and that call is not a constant expression"
//@ diag: 31:37 note "read of non-const variable 'g' is not allowed in a constant expression"
//@ diag: 33:8 note "in call to 'B()'"
//@ diag: 37:6 note "in call to 'E2()'"
//@ diag: 30:5 note "declared here"
//@ diag: 42:18 error "call to immediate function 'N::N' is not a constant expression"
//@ diag: 41:8 note "'N' is an immediate constructor because the default initializer of 'm' contains a call to a consteval constructor 'M' and that call is not a constant expression"
//@ diag: 42:21 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 42:11 note "declared here"
int g;
struct B { int v; consteval B() : v(g) {} };
struct E { B b; };
struct E2 : B {};
struct E3 { int x; B b; E3(int) {} };
void f() {
  E e;
  E2 e2;
  E3 e3(1);
}
struct M { int *p; constexpr M(int *q) : p(q) {} consteval M(const M &o) : p(o.p) {} };
struct N { M m; };
void h(N &n) { N n2(n); }
struct C { int *p; consteval C() : p(&g) {} };
struct F { C c; };
struct G : C { int k = 1; };
F f1;
G g1;
constexpr F f2{};
int *use() { return F().c.p; }
