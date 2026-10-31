// [class.abstract]/6, [class.cdtor]/4, [expr.const]/5.2, CWG1517: a virtual
// call whose final overrider (for the dynamic type during construction or
// destruction) is pure, a virtual call on a subobject whose construction
// has not started (no dynamic type yet), and on an object whose dynamic
// type is not known to the evaluation (clang's notes).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:15 error "constexpr variable 'x1' must be initialized by a constant expression"
//@ diag: 32:24 note "pure virtual function 'P::f' called"
//@ diag: 31:19 note "in call to 'this->g()'"
//@ diag: 35:8 note "in call to 'P()'"
//@ diag: 36:34 note "in call to 'Q()'"
//@ diag: 37:20 note "in call to 'pure_in_ctor()'"
//@ diag: 33:16 note "declared here"
//@ diag: 42:15 error "constexpr variable 'x2' must be initialized by a constant expression"
//@ diag: 40:60 note "member call on object outside its lifetime is not allowed in a constant expression"
//@ diag: 40:85 note "in call to 'this->vf()'"
//@ diag: 41:41 note "in call to 'S()'"
//@ diag: 42:20 note "in call to 'before_construction()'"
//@ diag: 46:15 error "constexpr variable 'x3' must be initialized by a constant expression"
//@ diag: 44:43 note "virtual function called on object 'ext' whose dynamic type is not constant"
//@ diag: 46:20 note "in call to 'call(ext)'"
//@ diag: 55:15 error "constexpr variable 'x4' must be initialized by a constant expression"
//@ diag: 50:24 note "pure virtual function 'PD::f' called"
//@ diag: 49:29 note "in call to 'this->h()'"
//@ diag: 54:49 note "in call to '{*new PDD#0}.~PD()'"
//@ diag: 54:49 note "in call to '{*new PDD#0}.~PDD()'"
//@ diag: 55:20 note "in call to 'pure_in_dtor()'"
//@ diag: 51:16 note "declared here"
struct P {
  constexpr P() { g(); }
  constexpr void g() { f(); }
  virtual void f() = 0;
};
struct Q : P { constexpr void f() override {} };
constexpr int pure_in_ctor() { Q q; return 0; }
constexpr int x1 = pure_in_ctor();

struct V { constexpr virtual int f() const { return 1; } };
struct S { int n; V v; constexpr int vf() const { return v.f(); } constexpr S() : n(vf()) {} };
constexpr int before_construction() { S s; return s.n; }
constexpr int x2 = before_construction();

constexpr int call(const V &v) { return v.f(); }
extern const V ext;
constexpr int x3 = call(ext);

struct PD {
  constexpr virtual ~PD() { h(); }
  constexpr void h() { f(); }
  virtual void f() = 0;
};
struct PDD : PD { constexpr void f() override {} };
constexpr int pure_in_dtor() { PD *p = new PDD; delete p; return 0; }
constexpr int x4 = pure_in_dtor();
