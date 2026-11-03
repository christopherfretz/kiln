// [over.match.class.deduct]/1, [over.match.best]: clang 21+ resolves the
// non-template guides first (clang's deferred template candidates); when
// the best is a perfect match (identity conversions, lvalue-to-rvalue
// aside) no template guide is deduced, so Bad<int> is never instantiated.
// A promotion (float -> double) is not perfect: every guide is deduced.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 13:46 error "static assertion failed due to requirement 'sizeof(float) == 0': deduced"
//@ diag: 15:33 note "in instantiation of template class 'Bad<float>' requested here"
//@ diag: 23:3 note "while substituting deduced template arguments into function template '<deduction guide for S>' [with T = float]"
//@ diag: 13:56 note "expression evaluates to '4 == 0'"
template<class T> struct Bad { static_assert(sizeof(T) == 0, "deduced"); using type = int; };
template<class T> struct S { S(T, int = 0); };
template<class T> S(T, typename Bad<T>::type) -> S<T>;
S(int, int) -> S<long>;
S(double, int) -> S<char>;
S s(1, 2);
S<long> *ps = &s;
const int ci = 3;
S t(ci, 2);
S<long> *pt = &t;
S u(1.0f, 2);
S<float> *pu = &u;
