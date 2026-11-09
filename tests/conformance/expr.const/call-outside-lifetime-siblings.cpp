// [expr.const]/10.8, [basic.life]/7.2: a member call on an object whose
// lifetime ended, by an explicit destructor call or by its scope ending
// (WP-948 siblings of call-out-of-lifetime).
//@ std: c++23
//@ expect: compile-fail
//@ diag: 21:15 error "constexpr variable 'k' must be initialized by a constant expression"
//@ diag: 19:12 note "member call on object outside its lifetime is not allowed in a constant expression"
//@ diag: 21:19 note "in call to 'f()'"
//@ diag: 30:15 error "constexpr variable 'm' must be initialized by a constant expression"
//@ diag: 28:13 note "member call on object outside its lifetime is not allowed in a constant expression"
//@ diag: 30:19 note "in call to 'g()'"
struct S {
  int x;
  constexpr int get() const { return x; }
};
constexpr int f() {
  S s{1};
  s.~S();
  return s.get();
}
constexpr int k = f();
constexpr int g() {
  const S *p;
  {
    S s{2};
    p = &s;
  }
  return p->get();
}
constexpr int m = g();
