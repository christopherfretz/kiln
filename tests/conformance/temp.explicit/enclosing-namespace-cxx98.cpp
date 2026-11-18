// C++98 [temp.explicit]/5 asked an explicit instantiation to appear in
// the template's namespace; C++11 relaxed that to an enclosing
// namespace, and clang (only a -Wc++11-compat warning, off by default)
// and GCC accept the C++11 form in C++98 too. Found running GCC's C++
// testsuite. With WP-1692 (the C++11 rule applied in C++98, see
// scope-not-enclosing-cxx98-forms.cpp), the valid forms: member functions,
// static data members, `extern template`, and a template in an inline
// namespace (a C++11 feature, an extension in C++98) instantiated in its
// enclosing namespace, unqualified or qualified.
//@ expect: compile-ok
//@ std: c++98
//@ oracle: gcc
//@ args: -Wno-c++11-extensions

namespace N {
template <class T> struct S { void f() {} };
template <class T> void g(T) {}
}
template struct N::S<int>;
template void N::g(int);

namespace N {
template <class T> struct C { void m() {} static int s; };
template <class T> int C<T>::s = 0;
inline namespace In { template <class T> struct G {}; }
}
template void N::C<int>::m();
template int N::C<int>::s;
extern template struct N::S<char>;
template struct N::G<int>;
namespace N {
template struct G<char>;
template struct In::G<long>;
template void C<long>::m();
}
