// GNU `static template` / `inline template` explicit instantiation
// ([temp.explicit] with a GNU storage keyword): clang 22 ignores the
// keyword with a warning, GCC 16.1 accepts it. Reduced from GCC's
// g++.dg/ext/instantiate2.C; found running GCC's C++ testsuite (WP-1466).
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
//@ diag: 12:1 warning "ignoring 'static' keyword on explicit template instantiation"
//@ diag: 13:1 warning "ignoring 'inline' keyword on explicit template instantiation"
template<class T> struct A { static T t; };
template<class T> T A<T>::t = 0;
static template struct A<int>;
inline template struct A<char>;
