// [over.match.class.deduct]/1, /3: an implicit deduction guide's notes print
// its parameter types as the constructor wrote them (member typedefs,
// dependent typename and template-ids, array references), an alias
// template's guide with the alias's arguments substituted into that sugar.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 42:3 error "no viable constructor or deduction guide for deduction of template arguments of 'A'"
//@ diag: 41:19 note "candidate template ignored: could not match 'V<T>' against 'int'"
//@ diag: 41:19 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(A, V<T>) A(V<T>) -> V<T>'"
//@ diag: 41:19 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 41:19 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(A, V<T>) A(typename T::value_type, size_type n) -> V<T>'"
//@ diag: 48:3 error "no viable constructor or deduction guide for deduction of template arguments of 'Y'"
//@ diag: 45:26 note "candidate template ignored: could not match 'Y<T>' against 'int'"
//@ diag: 45:26 note "implicit deduction guide declared as 'template <class T> Y(Y<T>) -> Y<T>'"
//@ diag: 46:3 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 46:3 note "implicit deduction guide declared as 'template <class T> Y(typename X<T>::type, typename T::template rebind<int>::other) -> Y<T>'"
//@ diag: 55:3 error "no viable constructor or deduction guide for deduction of template arguments of 'W'"
//@ diag: 50:26 note "candidate template ignored: could not match 'W<T>' against 'int'"
//@ diag: 50:26 note "implicit deduction guide declared as 'template <class T> W(W<T>) -> W<T>'"
//@ diag: 53:3 note "candidate function template not viable: requires 3 arguments, but 1 was provided"
//@ diag: 53:3 note "implicit deduction guide declared as 'template <class T> W(ref, ptr, int (&)[3]) -> W<T>'"
//@ diag: 57:4 error "no viable constructor or deduction guide for deduction of template arguments of 'WA'"
//@ diag: 56:19 note "candidate template ignored: could not match 'W<T>' against 'int'"
//@ diag: 56:19 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(WA, W<T>) WA(W<T>) -> W<T>'"
//@ diag: 56:19 note "candidate function template not viable: requires 3 arguments, but 1 was provided"
//@ diag: 56:19 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(WA, W<T>) WA(ref, ptr, int (&)[3]) -> W<T>'"
//@ diag: 59:3 error "no viable constructor or deduction guide for deduction of template arguments of 'P'"
//@ diag: 58:19 note "candidate function template not viable: requires 2 arguments, but 3 were provided"
//@ diag: 58:19 note "implicit deduction guide declared as 'template <class U> requires __is_deducible(P, V<X<U>>) P(typename X<U>::value_type, size_type n) -> V<X<U>>'"
//@ diag: 58:19 note "candidate function template not viable: requires 1 argument, but 3 were provided"
//@ diag: 58:19 note "implicit deduction guide declared as 'template <class U> requires __is_deducible(P, V<X<U>>) P(V<X<U>>) -> V<X<U>>'"
//@ diag: 61:3 error "no viable constructor or deduction guide for deduction of template arguments of 'Q'"
//@ diag: 60:19 note "candidate template ignored: could not match 'W<U *>' against 'int'"
//@ diag: 60:19 note "implicit deduction guide declared as 'template <class U> requires __is_deducible(Q, W<U *>) Q(W<U *>) -> W<U *>'"
//@ diag: 60:19 note "candidate function template not viable: requires 3 arguments, but 1 was provided"
//@ diag: 60:19 note "implicit deduction guide declared as 'template <class U> requires __is_deducible(Q, W<U *>) Q(ref, ptr, int (&)[3]) -> W<U *>'"
template<class T> struct V {
  using size_type = unsigned;
  V(typename T::value_type, size_type n) {}
};
template<class T> using A = V<T>;
A a{1};

template<class T> struct X { using type = T; };
template<class T> struct Y {
  Y(typename X<T>::type, typename T::template rebind<int>::other) {}
};
Y y{1};

template<class T> struct W {
  using ref = const T &;
  typedef T *ptr;
  W(ref, ptr, int (&)[3]) {}
};
W w{1};
template<class T> using WA = W<T>;
WA wa{1};
template<class U> using P = V<X<U>>;
P p{1, 2, 3};
template<class U> using Q = W<U *>;
Q q{1};
