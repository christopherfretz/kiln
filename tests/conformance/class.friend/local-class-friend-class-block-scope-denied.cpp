// [class.friend]/11: an unqualified friend class of a local class names a
// class of the innermost enclosing block scope (or of an enclosing local
// class), never one outside it: a namespace class `X` or `W` of that name,
// declared before or after, is not the friend, nor is a class of the
// block named differently; the friend class declared in the block is not
// found by name lookup before its declaration there, so `Hid *p;` names
// nothing (found running GCC's C++ testsuite; GCC agrees).
//@ skip(clang): clang searches past the block scope, so it befriends the namespace class `W`
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:37 error "'i' is a private member of 'L'"
//@ diag: 22:33 note "implicitly declared private here"
//@ diag: 32:37 error "'i' is a private member of 'L'"
//@ diag: 31:34 note "implicitly declared private here"
//@ diag: 34:54 error "'i' is a private member of 'L'"
//@ diag: 37:15 note "in instantiation of function template specialization 'W::g<L>' requested here"
//@ diag: 36:34 note "implicitly declared private here"
//@ diag: 41:3 error "unknown type name 'Hid'"

class X;
void f() {
  class L { friend class X; int i; };
  struct Z { int g(L &l) { return l.i; } };
}
class X { public: int g(); };
void k() {
  class L { friend class X; int i; };
}
struct W;
void m() {
  class L { friend struct W; int i; };
  struct V { int g(L &l) { return l.i; } };
}
struct W { template <class T> int g(T &l) { return l.i; } };
void n() {
  class L { friend struct W; int i; };
  L l; W w; w.g(l);
}
void h() {
  class L { friend class Hid; int i; };
  Hid *p;
}
