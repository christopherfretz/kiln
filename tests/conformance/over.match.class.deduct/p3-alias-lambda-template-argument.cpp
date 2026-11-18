// [over.match.class.deduct]/3: an alias template whose defining type-id
// gives a lambda-expression as a constant template argument
// (`A<T, []{}>`, a closure type of no captures is structural,
// [temp.param]/7) deduces through the guides of `A`; the closure object
// is substituted into each guide as a constant template argument of
// class type (it reached the constant evaluator still wrapped as a
// substituted template parameter). Found running GCC's C++ testsuite
// (WP-1519). Clang 19 finds no viable guide.
//@ std: c++20
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
template <class T, auto V> struct A {
  A() {}
  A(T) {}
};
template <class T> using AT = A<T, [] {}>;
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
template <class> struct first;
template <class T, auto V> struct first<A<T, V>> { using type = T; };
AT<int> a;
AT b = a;
AT c = 1;
AT d = 'x';
// Each use of `AT<int>` has its own closure type, so only the deduced
// first argument is compared.
static_assert(same<first<decltype(c)>::type, int>);
static_assert(same<first<decltype(d)>::type, char>);
template <class T> using AV = A<T, [] { return 1; }()>;
AV e = 2L;
static_assert(same<decltype(e), A<long, 1>>);
