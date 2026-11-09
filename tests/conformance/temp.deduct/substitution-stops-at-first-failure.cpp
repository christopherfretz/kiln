// [temp.deduct.general]/7: substitution proceeds in lexical order and stops
// at the first failure; the return type fails first, so it_is_a_trap<char>
// is never instantiated. From clang's
// CXX/temp/temp.fct.spec/temp.deduct/sfinae-1.cpp. Found by the clang
// negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-ok
template <typename T> struct it_is_a_trap {
  typedef typename T::trap type;
};
template <bool, typename T = void> struct enable_if {
  typedef T type;
};
template <typename T> struct enable_if<false, T> {};
template <typename T>
typename enable_if<sizeof(T) == 17>::type f(const T &, typename it_is_a_trap<T>::type * = 0);
void f(...);
void test_f() { f('a'); }
