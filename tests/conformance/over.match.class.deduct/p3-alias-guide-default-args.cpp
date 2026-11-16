// [over.match.class.deduct]/3: the guide of an alias template formed from a
// constructor with default arguments keeps them (fewer arguments deduce),
// and is printed without them, as clang prints the transformed guide.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 22:3 error "no viable constructor or deduction guide for deduction of template arguments of 'A'"
//@ diag: 19:20 note "candidate function template not viable: requires at most 3 arguments, but 4 were provided"
//@ diag: 19:20 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(A, C<T, char>) A(T, int, char *) -> C<T, char>'"
//@ diag: 19:20 note "candidate function template not viable: requires 2 arguments, but 4 were provided"
//@ diag: 19:20 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(A, C<T, char>) A(T, char) -> C<T, char>'"
//@ diag: 19:20 note "candidate function template not viable: requires 1 argument, but 4 were provided"
//@ diag: 19:20 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(A, C<T, char>) A(C<T, char>) -> C<T, char>'"

template <class T, class U> struct C {
  C(T, U);
  C(T, int = 0, U* = nullptr);
};
template <class T> using A = C<T, char>;
A a = 1.0;
static_assert(sizeof(a) == sizeof(C<double, char>));
A b{1, 2, 3, 4};
