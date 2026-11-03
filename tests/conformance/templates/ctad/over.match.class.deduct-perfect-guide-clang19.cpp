// [over.match.class.deduct]/1: clang 19 deduces every template guide,
// even beside a non-template guide that is a perfect match.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 14:46 error "static assertion failed due to requirement 'sizeof(int) == 0': deduced"
//@ diag: 16:33 note "in instantiation of template class 'Bad<int>' requested here"
//@ diag: 19:3 note "while substituting deduced template arguments into function template '<deduction guide for S>' [with T = int]"
//@ diag: 14:56 note "expression evaluates to '4 == 0'"
//@ diag: 14:46 error "static assertion failed due to requirement 'sizeof(float) == 0': deduced"
//@ diag: 16:33 note "in instantiation of template class 'Bad<float>' requested here"
//@ diag: 24:3 note "while substituting deduced template arguments into function template '<deduction guide for S>' [with T = float]"
//@ diag: 14:56 note "expression evaluates to '4 == 0'"
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
