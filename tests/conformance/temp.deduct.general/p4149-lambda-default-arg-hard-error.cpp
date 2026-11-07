// [temp.deduct.general]/8 (P4149R1, Croydon DR): the default arguments of a
// non-generic lambda's call operator are instantiated with the closure type
// but are outside the immediate context of the deduction that reached the
// lambda, so an invalid default argument is a hard error even though the
// call never uses it and a fallback overload exists.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:55 error "no matching conversion for functional-style cast from 'int' to 'X'"
//@ diag: 15:55 note "in instantiation of default function argument expression for 'operator()<X>' required here"
//@ diag: 17:10 note "while substituting deduced template arguments into function template 'foo' [with T = X]"
//@ diag: 14:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const X' for 1st argument"
//@ diag: 14:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'X' for 1st argument"
//@ diag: 14:8 note "candidate constructor (the implicit default constructor) not viable: requires 0 arguments, but 1 was provided"
struct X {};
template <class T> auto foo(T t) -> decltype([](T x = T(1)) { return 0; }(t));
long foo(...);
auto r = foo(X{});
