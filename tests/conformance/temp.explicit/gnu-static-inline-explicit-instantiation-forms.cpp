// GNU `static template` / `inline template` explicit instantiations of a
// class, a function, a static data member and a member function: the
// keyword is ignored with -Wstatic-inline-explicit-instantiation at it
// (clang 22; not a pedantic diagnostic); `extern template` is the standard
// explicit instantiation declaration (WP-1495).
//@ expect: compile-ok
//@ std: c++17
//@ args: -pedantic-errors
//@ strict-diags
//@ diag: 18:1 warning "ignoring 'static' keyword on explicit template instantiation"
//@ diag: 19:1 warning "ignoring 'inline' keyword on explicit template instantiation"
//@ diag: 21:1 warning "ignoring 'static' keyword on explicit template instantiation"
//@ diag: 22:1 warning "ignoring 'inline' keyword on explicit template instantiation"
//@ diag: 23:1 warning "ignoring 'static' keyword on explicit template instantiation"
//@ diag: 24:1 warning "ignoring 'static' keyword on explicit template instantiation"
template<class T> struct A { static T t; void f() {} }; template<class T> void g(T) {}
template<class T> T A<T>::t = 0;
static template struct A<int>;
inline template struct A<char>;
extern template struct A<long>;
static template void g(int);
inline template void g(char);
static template short A<short>::t;
static template void A<double>::f();
