// [conv.lval]/3.2: an ellipsis argument in an unevaluated operand copies
// nothing; [class.protected]: a protected member named through the
// context's class is accessible without an object.
//@ std: c++17
struct A {
  A() = default;
private:
  A(A const&) = default;
};
int f(...);
void m() { A a; (void)sizeof(f(a)); }
struct P { protected: void foo(); int x; };
struct Q : P {
  void bar() { void (P::*pmf)() = &Q::foo; (void)pmf; (void)&Q::x; (void)&x; }
};
