// [temp.deduct.general]/8 (P4149R1, Croydon DR): a default argument is a
// separately instantiated construct, outside the immediate context. A call
// in the return type that needs `g`'s default argument instantiates it, and
// the error there is a hard error rather than a substitution failure of `f`.
// (clang 19 and 22 agree; GCC 16.1 still treats it as a substitution
// failure.)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:35 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 13:44 note "in instantiation of default function argument expression for 'g<int>' required here"
//@ diag: 15:20 note "while substituting deduced template arguments into function template 'f' [with T = int]"
template <class T> int g(T, int = T::x);
template <class T> auto f(T t) -> decltype(g(t));
long f(...);
using R = decltype(f(1));
