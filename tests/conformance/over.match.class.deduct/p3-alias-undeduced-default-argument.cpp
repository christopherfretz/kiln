//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 33:10 error "no viable constructor or deduction guide for deduction of template arguments of 'N'"
//@ diag: 32:20 note "candidate template ignored: constraints not satisfied"
//@ diag: 32:20 note "cannot deduce template arguments for 'N' from 'Q<int>'"
//@ diag: 32:20 note "implicit deduction guide declared as 'template <> requires __is_deducible(N, Q<int>) N(int) -> Q<int>'"
//@ diag: 32:20 note "candidate template ignored: constraints not satisfied"
//@ diag: 32:20 note "cannot deduce template arguments for 'N' from 'Q<int>'"
//@ diag: 32:20 note "implicit deduction guide declared as 'template <> requires __is_deducible(N, Q<int>) N(Q<int>) -> Q<int>'"
// [over.match.class.deduct]/3-4: an alias template parameter that
// deduction from the guide's return type leaves undeduced is deducible
// when it has a default argument ([temp.deduct.general]/5); one without
// a default is not. Found running GCC's C++ testsuite.

template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

template <class> struct A { A(int = 0); };
template <class = void> using B = A<int>;
auto x = B{};
auto y = B();
auto z = B{1};
auto w = B(1);
static_assert(same<decltype(x), A<int>> && same<decltype(w), A<int>>);

template <class T, class U = T*> using P = A<T>;
auto p = P(A<long>(1));
static_assert(same<decltype(p), A<long>>);

template <class> struct Q { Q(int); };
template <class T> using N = Q<int>;
auto n = N(1);
