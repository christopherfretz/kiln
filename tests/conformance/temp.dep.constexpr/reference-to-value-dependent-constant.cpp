// [temp.dep.constexpr]/2: i is a reference initialized with the value-
// dependent constant k, so S<i> is value-dependent and is not checked before
// instantiation. From clang's
// CXX/temp/temp.res/temp.dep/temp.dep.constexpr/p2.cpp. Found by the clang
// negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-ok
template <int n> struct S;
template <int n> struct T {
  T() {
    const int k = n;
    const int &i = k;
    typename S<i>::T check4;
  }
};
