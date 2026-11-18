// [module.unit]/7, [basic.link]/11: an out-of-line definition of a member
// is attached to the module in whose purview it is written, so defining
// in module M's purview a member of a class declared in `extern "C++"`
// (attached to the global module) is ill-formed: a destructor, a member
// function, a static data member, a nested class, a member template and
// a befriended function alike. Inside `extern "C++" { }` in the purview
// the definitions are attached to the global module too, and valid. Same
// errors as clang 22 (GCC 16.1 rejects the same definitions); kiln
// diagnosed only the friend.
//@ expect: compile-fail
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 42:4 error "declaration of '~X' in module M follows declaration in the global module"
//@ diag: 29:3 note "previous declaration is here"
//@ diag: 43:9 error "declaration of 'f' in module M follows declaration in the global module"
//@ diag: 30:8 note "previous declaration is here"
//@ diag: 44:8 error "declaration of 's' in module M follows declaration in the global module"
//@ diag: 31:14 note "previous declaration is here"
//@ diag: 45:11 error "declaration of 'N' in module M follows declaration in the global module"
//@ diag: 32:10 note "previous declaration is here"
//@ diag: 46:28 error "declaration of 'g' in module M follows declaration in the global module"
//@ diag: 33:27 note "previous declaration is here"
//@ diag: 47:6 error "declaration of 'h' in module M follows declaration in the global module"
//@ diag: 34:15 note "previous declaration is here"
export module M;
extern "C++" {
struct X {
  ~X();
  void f();
  static int s;
  struct N;
  template <class T> void g(T);
  friend void h(X);
};
struct Y {
  void f();
  static int s;
  struct N;
};
}
X::~X() {}
void X::f() {}
int X::s = 1;
struct X::N {};
template <class T> void X::g(T) {}
void h(X) {}
extern "C++" {
void Y::f() {}
int Y::s = 2;
struct Y::N {};
}
struct Z {
  void f();
};
void Z::f() {}
