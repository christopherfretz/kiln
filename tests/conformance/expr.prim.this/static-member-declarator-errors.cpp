// [expr.prim.this]/2-3: `this` may not appear in the declaration of a
// static member function: a member named without an object in its trailing
// return type, *noexcept-specifier* or *requires-clause* is an implicit
// `this` there (clang `checkThisInStaticMemberFunctionType`), also for a
// static member function template, an out-of-line definition (static only
// by its earlier declaration) and each instantiation of a class template's
// member. A default argument has no `this` at all: the call has no
// object. Found running GCC's C++ testsuite.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:31 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 35:50 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 36:53 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 37:31 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 38:31 error "'this' cannot be used in a static member function declaration"
//@ diag: 32:22 error "call to non-static member function without an object argument"
//@ diag: 33:36 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 42:25 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 43:44 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 47:31 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 48:31 error "'this' cannot be used in a static member function declaration"
//@ diag: 47:31 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 50:9 note "in instantiation of template class 'HT<int>' requested here"
//@ diag: 48:31 error "'this' cannot be used in a static member function declaration"
//@ diag: 55:29 error "invalid use of 'this' in a function with an explicit object parameter"
//@ diag: 56:29 error "invalid use of member 'm' in explicit object member function"
//@ diag: 57:29 error "call to non-static member function without an object argument"
struct S {
  int m;
  int f();
  static int a(int = f());
  static int b() noexcept(noexcept(f()));
  static auto c() -> decltype(f());
  template <class T> static auto d() -> decltype(f());
  template <class T> static int e() requires(sizeof(f()) > 0);
  static auto h() -> decltype(m);
  static auto k() -> decltype(this);
  static auto g() -> int;
  template <class T> static auto t() -> int;
};
auto S::g() -> decltype(f()) { return 0; }
template <class T> auto S::t() -> decltype(f()) { return 0; }
template <class U> struct HT {
  int m;
  int f();
  static auto g() -> decltype(f());
  static auto k() -> decltype(this->m);
};
HT<int> ht;
// An explicit object member function has no `this` in its body.
struct X {
  int m;
  int f();
  int a(this X& s) { return this->m; }
  int b(this X& s) { return m; }
  int c(this X& s) { return f(); }
};
