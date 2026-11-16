// [temp.deduct.call]/1 with [over.match.class.deduct]/1.6.1: a nested
// braced-init-list for a multi-dimensional array element is deduced
// element by element against `T[N]`, recursively; brace elision of the
// same arrays forms one parameter per scalar. A braced element that
// deduces two different types is no candidate. Found running GCC's C++
// testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ diag: 27:3 error "no viable constructor or deduction guide for deduction of template arguments of 'E'"
//@ diag: 17:27 note "candidate template ignored: couldn't infer template argument 'T'"
//@ diag: 17:27 note "implicit deduction guide declared as 'template <class T> E(E<T>) -> E<T>'"
//@ diag: 17:27 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'long')"
//@ diag: 17:27 note "implicit deduction guide declared as 'template <class T> E(T (&&)[3][2]) -> E<T>'"
//@ diag: 17:27 note "candidate function template not viable: requires 0 arguments, but 1 was provided"
//@ diag: 17:27 note "implicit deduction guide declared as 'template <class T> E() -> E<T>'"
template <class T> struct E { T t[3][2]; };
template <class T> struct F { T t[2][3]; };
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
E e1{1, 2, 3, 4, 5, 6};
E e2{{{1, 2}, {3, 4}, {5, 6}}};
F f1{1, 2, 3, 4, 5, 6};
F f2{{{1, 2, 3}, {4, 5, 6}}};
static_assert(same<decltype(e1), E<int>> && same<decltype(e2), E<int>>);
static_assert(same<decltype(f1), F<int>> && same<decltype(f2), F<int>>);
E bad{{{1, 2}, {3L, 4L}, {5, 6}}};
