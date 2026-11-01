// [temp.point]/7, [temp.inst]/11: definitions instantiated at the end of the
// translation unit that require further instantiations get them right away,
// inside their own instantiation (clang `PerformPendingInstantiations` from
// `InstantiateFunctionDefinition`), so an error in a nested one carries the
// requesting definition's "in instantiation of ... requested here" too.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:34 error "no member named 'nope' in 'X'"
//@ diag: 36:31 note "in instantiation of function template specialization 'h<X>' requested here"
//@ diag: 37:31 note "in instantiation of function template specialization 'g<X>' requested here"
//@ diag: 38:14 note "in instantiation of function template specialization 'k<X>' requested here"
//@ diag: 40:45 error "no member named 'nope2' in 'X'"
//@ diag: 40:65 note "in instantiation of member function 'S<X>::f' requested here"
//@ diag: 41:23 note "in instantiation of member function 'S<X>::e' requested here"
//@ diag: 42:53 error "no member named 'nope3' in 'X'"
//@ diag: 43:37 note "in instantiation of member function 'V<X>::v' requested here"
//@ diag: 44:15 note "in instantiation of function template specialization 'mk<X>' requested here"
//@ diag: 46:32 error "no member named 'nope4' in 'X'"
//@ diag: 47:38 note "in instantiation of variable template specialization 'vt<X>' requested here"
//@ diag: 48:15 note "in instantiation of function template specialization 'vg<X>' requested here"
//@ diag: 50:63 error "no member named 'nope5' in 'X'"
//@ diag: 50:75 note "in instantiation of function template specialization 'r<X, 0>' requested here"
//@ diag: 50:75 note "in instantiation of function template specialization 'r<X, 1>' requested here"
//@ diag: 50:75 note "in instantiation of function template specialization 'r<X, 2>' requested here"
//@ diag: 50:75 note "in instantiation of function template specialization 'r<X, 3>' requested here"
//@ diag: 50:75 note "in instantiation of function template specialization 'r<X, 4>' requested here"
//@ diag: 50:75 note "(skipping 5 contexts in backtrace; use -ftemplate-backtrace-limit=0 to see all)"
//@ diag: 50:75 note "in instantiation of function template specialization 'r<X, 10>' requested here"
//@ diag: 50:75 note "in instantiation of function template specialization 'r<X, 11>' requested here"
//@ diag: 50:75 note "in instantiation of function template specialization 'r<X, 12>' requested here"
//@ diag: 50:75 note "in instantiation of function template specialization 'r<X, 13>' requested here"
//@ diag: 51:15 note "in instantiation of function template specialization 'r<X, 14>' requested here"
struct X {};
// A chain of function template specializations.
template <class T> void h() { T::nope; }
template <class T> void g() { h<T>(); }
template <class T> void k() { g<T>(); }
int main() { k<X>(); }
// Member functions, and a virtual member a vtable use requires.
template <class T> struct S { void f() { T::nope2; } void e() { f(); } };
void use(S<X> &s) { s.e(); }
template <class T> struct V { virtual void v() { T::nope3; } };
template <class T> void mk() { V<T> v; }
void use2() { mk<X>(); }
// A variable template specialization.
template <class T> int vt = T::nope4;
template <class T> void vg() { (void)vt<T>; }
void use3() { vg<X>(); }
// A long chain is elided like any other backtrace.
template <class T, int N> void r() { if constexpr (N == 0) T::nope5; else r<T, N - 1>(); }
void use4() { r<X, 14>(); }
