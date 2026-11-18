// [expr.prim.lambda.capture]/11, [basic.def.odr]/5, [expr.const]/10: the
// forms of a constant expression in a lambda body naming a by-copy capture
// that kiln keeps rejecting, as the text does (the name denotes the
// closure's member). Where clang 22 and GCC 16.1 both accept, kiln reads
// the captured variable instead (constexpr-call-reads-by-copy-capture-
// run.cpp, WP-1655); these are the forms where they differ or both reject.
// GCC 16.1 rejects and clang 22 accepts: a read in place (`(*&p).v`, `*a`),
// a generic lambda, a trivial copy in a parenthesized initializer, a
// reference member bound to it. Both reject: an init-capture (its own
// variable), a pointer or a reference to it, a `const` object that is not
// usable in constant expressions, a copy by a non-constexpr constructor
// (GCC 16.1 accepts that one alone).
//@ xfail(clang): clang 19 and 22 evaluate the captured variable in every form
//@ expect: compile-fail
//@ args: -std=c++26 -Wno-unused-lambda-capture -Wno-unused-variable
//@ diag: 69:19 error "constexpr variable 'j1' must be initialized by a constant expression"
//@ diag: 69:24 note "kiln: read of variable 'p' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 64:15 note "declared here"
//@ diag: 70:19 error "constexpr variable 'j2' must be initialized by a constant expression"
//@ diag: 70:24 note "kiln: read of variable 'a' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 65:17 note "declared here"
//@ diag: 71:17 error "constexpr variable 'c1' must be initialized by a constant expression"
//@ diag: 71:17 note "kiln: read of variable 'p' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 71:17 note "in call to 'P(p)'"
//@ diag: 64:15 note "declared here"
//@ diag: 72:19 error "constexpr variable 'j3' must be initialized by a constant expression"
//@ diag: 72:24 note "kiln: read of variable 'p' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 64:15 note "declared here"
//@ diag: 73:24 error "constexpr variable 'p1' must be initialized by a constant expression"
//@ diag: 73:24 note "pointer to 'p' is not a constant expression"
//@ diag: 64:15 note "address of non-static constexpr variable 'p' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 74:24 error "constexpr variable 'r1' must be initialized by a constant expression"
//@ diag: 74:24 note "reference to 'p' is not a constant expression"
//@ diag: 64:15 note "address of non-static constexpr variable 'p' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 75:19 error "constexpr variable 'j4' must be initialized by a constant expression"
//@ diag: 53:38 note "read of non-constexpr variable 'c' is not allowed in a constant expression"
//@ diag: 75:24 note "in call to 'c.get()'"
//@ diag: 67:11 note "declared here"
//@ diag: 76:19 error "constexpr variable 'j5' must be initialized by a constant expression"
//@ diag: 76:30 note "non-constexpr constructor 'Q' cannot be used in a constant expression"
//@ diag: 58:3 note "declared here"
//@ diag: 78:29 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 53:38 note "kiln: read of variable 'p' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 78:33 note "in call to 'p.get()'"
//@ diag: 64:15 note "declared here"
//@ diag: 79:27 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 53:38 note "read of non-constexpr variable 'p' is not allowed in a constant expression"
//@ diag: 79:31 note "in call to 'p.get()'"
//@ diag: 79:4 note "declared here"
struct P {
  int v;
  int a[2];
  constexpr int get() const { return v; }
};
struct Q {
  int v;
  constexpr Q(int x) : v(x) {}
  Q(const Q& o) : v(o.v) {}
};
struct V { const P& r; };
constexpr int byval(Q q) { return q.v; }

void forms() {
  constexpr P p{3, {4, 5}};
  constexpr int a[2] = {1, 2};
  constexpr Q q{3};
  const P c{3, {4, 5}};
  [=] {
    constexpr int j1 = (*&p).v;
    constexpr int j2 = *a;
    constexpr P c1(p);
    constexpr int j3 = V{p}.r.v;
    constexpr const P* p1 = &p;
    constexpr const P& r1 = p;
    constexpr int j4 = c.get();
    constexpr int j5 = byval(q);
  }();
  [=](auto) { constexpr int j = p.get(); return j; }(0);
  [p = p] { constexpr int j = p.get(); return j; }();
}
