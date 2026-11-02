// [class.friend]/6, [dcl.stc]/1: no storage-class-specifier may appear in a
// friend declaration. Each one (and thread_local, named with the storage
// class when both appear, e.g. 'static thread_local') is diagnosed
// "'X' is invalid in friend declarations" at the specifier, which is then
// dropped: the friend declaration proceeds (a friend function stays valid
// and callable, a friend typedef or variable is still "friends can only be
// classes or functions"), in either specifier order (WP-293).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:10 error "'mutable' is invalid in friend declarations"
//@ diag: 32:10 error "'mutable' is invalid in friend declarations"
//@ diag: 33:3 error "'mutable' is invalid in friend declarations"
//@ diag: 34:10 error "'static' is invalid in friend declarations"
//@ diag: 35:10 error "'extern' is invalid in friend declarations"
//@ diag: 36:10 error "'thread_local' is invalid in friend declarations"
//@ diag: 37:10 error "'__thread' is invalid in friend declarations"
//@ diag: 38:17 error "'static thread_local' is invalid in friend declarations"
//@ diag: 39:10 error "'register' is invalid in friend declarations"
//@ diag: 40:10 error "'mutable' is invalid in friend declarations"
//@ diag: 41:10 error "'mutable' is invalid in friend declarations"
//@ diag: 42:29 error "'mutable' is invalid in friend declarations"
//@ diag: 43:10 error "'mutable' is invalid in friend declarations"
//@ diag: 44:10 error "'typedef' is invalid in friend declarations"
//@ diag: 44:22 error "friends can only be classes or functions"
//@ diag: 45:10 error "'register' is invalid in friend declarations"
//@ diag: 45:23 error "friends can only be classes or functions"
//@ diag: 52:13 error "no member named 'v' in 'S'"

struct A;
struct S {
  friend mutable void f();
  friend mutable int g(int);
  mutable friend void h();
  friend static void i();
  friend extern void j();
  friend thread_local void k();
  friend __thread void l();
  friend static thread_local void m();
  friend register void n();
  friend mutable struct A;
  friend mutable class C;
  template <class T> friend mutable void t(T);
  friend mutable void d() {}
  friend typedef int T;
  friend register int v;
};
void f() {}
void d();
int use() {
  d();
  f();
  return S::v, 0;
}
