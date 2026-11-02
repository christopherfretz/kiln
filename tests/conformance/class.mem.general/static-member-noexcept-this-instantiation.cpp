// [expr.prim.this]/3, [temp.inst]/3, [except.spec]/13: `this` in the
// noexcept-specifier of a static member function of a class template is
// diagnosed at the definition and again in each instantiation of the class
// (the first error per instantiation carrying its context note). When the
// specifier itself is instantiated, an implicit `this` there is an "invalid
// use of 'this' outside of a non-static member function" (clang
// `checkThisInStaticMemberFunctionExceptionSpec`, `InstantiateExceptionSpec`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:37 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 23:37 error "'this' cannot be used in a static member function declaration"
//@ diag: 22:37 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 29:8 note "in instantiation of template class 'S<int>' requested here"
//@ diag: 23:37 error "'this' cannot be used in a static member function declaration"
//@ diag: 22:37 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 30:9 note "in instantiation of template class 'S<long>' requested here"
//@ diag: 23:37 error "'this' cannot be used in a static member function declaration"
//@ diag: 22:37 error "invalid use of 'this' outside of a non-static member function"
//@ diag: 33:11 note "in instantiation of exception specification for 'a' requested here"

template <class T> struct S {
  static void a() noexcept(noexcept(g()));
  static void b() noexcept(noexcept(this->g()));
  static void ok() noexcept(noexcept(s()));
  bool g();
  static bool s() noexcept;
};

S<int> s1;
S<long> s2;

void use() {
  S<int>::a();
  S<int>::ok();
}
static_assert(noexcept(S<long>::ok()));
