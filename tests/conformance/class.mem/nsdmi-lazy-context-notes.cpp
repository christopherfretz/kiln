// [class.mem.general]/8, [temp.inst]/3, [except.spec]/7, [class.inhctor.init]/1:
// the context notes of a default member initializer instantiated on first use,
// as clang orders them (`InstantiateInClassInitializer` inside
// `EvaluateImplicitExceptionSpec`, which `getSubobjectLoc` nests).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:53 error "no member named 'nope' in 'X'"
//@ diag: 32:38 note "in instantiation of default member initializer 'O<X>::I::v' requested here"
//@ diag: 32:64 note "in evaluation of exception specification for 'O<X>::I::I' needed here"
//@ diag: 33:6 note "in evaluation of exception specification for 'O<X>::O' needed here"
//@ diag: 34:42 error "no member named 'nope' in 'X'"
//@ diag: 34:27 note "in instantiation of default member initializer 'A<X>::v' requested here"
//@ diag: 35:17 note "in evaluation of exception specification for 'A<X>::A' needed here"
//@ diag: 35:22 note "in evaluation of exception specification for 'U::U' needed here"
//@ diag: 36:42 error "no member named 'nope' in 'X'"
//@ diag: 36:27 note "in instantiation of default member initializer 'C<X>::v' requested here"
//@ diag: 37:17 note "in evaluation of exception specification for 'C<X>::C' needed here"
//@ diag: 38:19 note "in evaluation of exception specification for 'Z::Z' needed here"
//@ diag: 40:42 error "no member named 'nope' in 'X'"
//@ diag: 40:58 note "in instantiation of default member initializer 'B<X>::v' requested here"
//@ diag: 41:26 note "in instantiation of member function 'B<X>::B' requested here"
//@ diag: 44:42 error "no member named 'nope' in 'X'"
//@ diag: 44:27 note "in instantiation of default member initializer 'R<X>::v' requested here"
//@ diag: 45:36 note "in evaluation of exception specification for 'R<X>::R' needed here"
//@ diag: 46:15 note "in instantiation of function template specialization 'g<X>' requested here"
//@ diag: 47:42 error "no member named 'nope' in 'X'"
//@ diag: 47:48 note "in instantiation of default member initializer 'H<X>::v' requested here"
//@ diag: 49:3 note "in instantiation of member function 'H<X>::H' requested here"
struct X {};
// Nested: each implicit exception specification evaluated for a subobject adds
// its own context, located at the member that calls it.
template <class T> struct O { struct I { int v = T::nope; }; I i; };
O<X> o;
template <class T> struct A { int v = T::nope; };
struct U { A<X> a; } u;
template <class T> struct C { int v = T::nope; };
struct Z { C<X> c[2]; };
bool b = noexcept(Z());
// A constexpr constructor used in a constant expression.
template <class T> struct B { int v = T::nope; constexpr B() {} };
constexpr int f() { B<X> b; return 0; }
// Through a member, a function template, and an inherited constructor (the
// base constructor is requested at the use).
template <class T> struct R { int v = T::nope; };
template <class T> void g() { R<T> r; }
template void g<X>();
template <class T> struct H { int v = T::nope; H(int) {} };
struct Y : H<X> { using H<X>::H; };
Y y(1);
