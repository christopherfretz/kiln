// [temp.explicit]/1: an explicit instantiation cannot be `inline`, and
// that of a function cannot be `constexpr` or `consteval` (clang words
// both as 'constexpr'); a static data member may say `constexpr`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:10 error "explicit instantiation cannot be 'constexpr'"
//@ diag: 14:10 error "explicit instantiation cannot be 'inline'"
//@ diag: 15:10 error "explicit instantiation cannot be 'constexpr'"
//@ diag: 18:10 error "explicit instantiation cannot be 'inline'"
//@ diag: 20:17 error "explicit instantiation cannot be 'inline'"
//@ diag: 22:10 error "explicit instantiation cannot be 'constexpr'"
template <class T> constexpr T sq(T v) { return v * v; }
template constexpr short sq<short>(short);
template inline int sq<int>(int);
template consteval long sq<long>(long);
template <class T> struct S { void f() {} S() {} static const int x; };
template <class T> const int S<T>::x = 1;
template inline void S<int>::f();
template constexpr const int S<int>::x;
extern template inline char sq<char>(char);
template void S<char>::f();
template constexpr S<char>::S();
