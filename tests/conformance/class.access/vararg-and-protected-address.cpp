// [conv.lval]/3.2, [class.access]: a class glvalue passed to an ellipsis
// copy-initializes a temporary, whose copy constructor must be accessible
// in an evaluated operand; [class.protected]: `&B::m` names a protected
// member without an object, so the naming class must be the context's
// class or derived from it. Found running GCC's C++ testsuite.
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:19 error "calling a private constructor of class 'A'"
//@ diag: 18:3 note "declared private here"
//@ diag: 24:39 error "'foo' is a protected member of 'P'"
//@ diag: 22:28 note "must name member using the type of the current context 'Q'"
//@ diag: 24:65 error "'x' is a protected member of 'P'"
//@ diag: 22:39 note "must name member using the type of the current context 'Q'"
struct A {
  A() = default;
private:
  A(A const&) = default;
};
int f(...);
void m() { A a; f(a); }
struct P { protected: void foo(); int x; };
struct Q : P {
  void bar() { void (P::*pmf)() = &P::foo; (void)pmf; (void)&P::x; }
};
