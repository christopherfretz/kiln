// [over.match.class.deduct]/1.5 (P1816): when no aggregate candidate is
// formed (too many clauses, a user guide exists, a non-trailing pack, a
// paren list cannot elide) or it fails deduction, the diagnostics and
// note order match clang's.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 50:5 error "no viable constructor or deduction guide for deduction of template arguments of 'Agg'"
//@ diag: 49:35 note "candidate function template not viable: requires 1 argument, but 3 were provided"
//@ diag: 49:35 note "implicit deduction guide declared as 'template <class T, class U> Agg(Agg<T, U>) -> Agg<T, U>'"
//@ diag: 49:35 note "candidate function template not viable: requires 0 arguments, but 3 were provided"
//@ diag: 49:35 note "implicit deduction guide declared as 'template <class T, class U> Agg() -> Agg<T, U>'"
//@ diag: 53:5 error "no viable constructor or deduction guide for deduction of template arguments of 'One'"
//@ diag: 51:26 note "candidate template ignored: could not match 'One<T>' against 'int'"
//@ diag: 51:26 note "implicit deduction guide declared as 'template <class T> One(One<T>) -> One<T>'"
//@ diag: 52:19 note "candidate template ignored: could not match 'T *' against 'int'"
//@ diag: 51:26 note "candidate function template not viable: requires 0 arguments, but 1 was provided"
//@ diag: 51:26 note "implicit deduction guide declared as 'template <class T> One() -> One<T>'"
//@ diag: 55:5 error "no viable constructor or deduction guide for deduction of template arguments of 'Two'"
//@ diag: 54:26 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'double')"
//@ diag: 54:26 note "implicit deduction guide declared as 'template <class T> Two(T, T) -> Two<T>'"
//@ diag: 54:26 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 54:26 note "implicit deduction guide declared as 'template <class T> Two(Two<T>) -> Two<T>'"
//@ diag: 54:26 note "candidate function template not viable: requires 0 arguments, but 2 were provided"
//@ diag: 54:26 note "implicit deduction guide declared as 'template <class T> Two() -> Two<T>'"
//@ diag: 57:6 error "no viable constructor or deduction guide for deduction of template arguments of 'Priv'"
//@ diag: 56:26 note "candidate template ignored: could not match 'Priv<T>' against 'int'"
//@ diag: 56:26 note "implicit deduction guide declared as 'template <class T> Priv(Priv<T>) -> Priv<T>'"
//@ diag: 56:26 note "candidate function template not viable: requires 0 arguments, but 1 was provided"
//@ diag: 56:26 note "implicit deduction guide declared as 'template <class T> Priv() -> Priv<T>'"
//@ diag: 60:3 error "no viable constructor or deduction guide for deduction of template arguments of 'A'"
//@ diag: 58:30 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 58:30 note "implicit deduction guide declared as 'template <class ...Ts> A(A<Ts...>) -> A<Ts...>'"
//@ diag: 58:30 note "candidate function template not viable: requires 0 arguments, but 2 were provided"
//@ diag: 58:30 note "implicit deduction guide declared as 'template <class ...Ts> A() -> A<Ts...>'"
//@ diag: 62:4 error "no viable constructor or deduction guide for deduction of template arguments of 'P1'"
//@ diag: 61:26 note "candidate template ignored: could not match 'P1<T>' against 'int'"
//@ diag: 61:26 note "implicit deduction guide declared as 'template <class T> P1(P1<T>) -> P1<T>'"
//@ diag: 61:26 note "candidate template ignored: could not match 'T *' against 'int'"
//@ diag: 61:26 note "implicit deduction guide declared as 'template <class T> P1(T *) -> P1<T>'"
//@ diag: 61:26 note "candidate function template not viable: requires 0 arguments, but 1 was provided"
//@ diag: 61:26 note "implicit deduction guide declared as 'template <class T> P1() -> P1<T>'"
//@ diag: 64:4 error "no viable constructor or deduction guide for deduction of template arguments of 'P2'"
//@ diag: 63:26 note "candidate template ignored: could not match 'T *' against 'int'"
//@ diag: 63:26 note "implicit deduction guide declared as 'template <class T> P2(T *, T *) -> P2<T>'"
//@ diag: 63:26 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 63:26 note "implicit deduction guide declared as 'template <class T> P2(P2<T>) -> P2<T>'"
//@ diag: 63:26 note "candidate function template not viable: requires 0 arguments, but 2 were provided"
//@ diag: 63:26 note "implicit deduction guide declared as 'template <class T> P2() -> P2<T>'"
template<class T, class U> struct Agg { T a; U b; };
Agg g{1, 2, 3};
template<class T> struct One { T v; };
template<class T> One(T *) -> One<T>;
One o{1};
template<class T> struct Two { T a, b; };
Two t{1, 2.0};
template<class T> struct Priv { private: T a; };
Priv p{1};
template<class... Ts> struct A : Ts... { int x; };
struct E {};
A a{E{}, 1};
template<class T> struct P1 { T *p; };
P1 q{1};
template<class T> struct P2 { T *p; T *r; };
P2 q2(1, 2);
