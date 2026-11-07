// [except.spec], [diff.cpp14.except]: from C++17 a dynamic exception
// specification with a pack expansion is diagnosed like any other
// non-empty one (clang's -Wdynamic-exception-spec, an error by default),
// on the template and on the class template member (WP-722).
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:33 error "ISO C++17 does not allow dynamic exception specifications"
//@ diag: 13:33 note "use 'noexcept(false)' instead"
//@ diag: 15:12 error "ISO C++17 does not allow dynamic exception specifications"
//@ diag: 15:12 note "use 'noexcept(false)' instead"
struct A {};
template <class... Ts> void f() throw(Ts...) {}
template <class... Ts> struct S {
  void g() throw(Ts..., int) {}
};
void h() { f<A>(); S<A>().g(); }
