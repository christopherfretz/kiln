// C++98 [temp.explicit]/5 asked an explicit instantiation to appear in
// the template's namespace; C++11 relaxed that to an enclosing
// namespace, and clang (only a -Wc++11-compat warning, off by default)
// and GCC accept the C++11 form in C++98 too. Found running GCC's C++
// testsuite.
//@ expect: compile-ok
//@ std: c++98

namespace N {
template <class T> struct S { void f() {} };
template <class T> void g(T) {}
}
template struct N::S<int>;
template void N::g(int);
