// [expr.call]: no -Winfinite-recursion when some live path reaches the exit
// without the call (a return, a throw, a loop that may not run), when the
// exit has no predecessor (an infinite loop, a noreturn call ahead of the
// call), for a virtual call through another object, a call qualified by a
// class template specialization, a function template or its
// specializations, a generic member, and an uninstantiated class template
// (clang checkRecursiveFunction; spec/sema.md §7.5.3).
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wall
[[noreturn]] void die();
int g1(int n) { if (n) return 0; return g1(n); }
int g2(int n) { if (n) throw 1; return g2(n); }
int g3(int n) { while (n) g3(n); return 0; }
void g4(int n) { for (int i = 0; i < n; ++i) g4(n); }
void g5() { while (true) {} g5(); }
void g6() { die(); g6(); }
void g7(int n) { (void)(n && (g7(1), true)); }
struct S {
  virtual void v(S *p) { p->v(p); }
  virtual void w(S &o) { o.w(o); }
};
template <class T> void t1() { t1<T>(); }
template <> void t1<char>() { t1<char>(); }
void use1() { t1<int>(); }
template <class T> struct C {
  static void s() { C<T>::s(); }
};
template struct C<int>;
struct D {
  void g(this auto &self) { self.g(); }
};
void use2() { D d; d.g(); }
template <class T> struct E { void q() { q(); } };
