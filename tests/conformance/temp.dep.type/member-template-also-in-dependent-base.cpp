// [temp.names]/7.3.2, [basic.lookup.qual.general]/3, [temp.res.general]/6:
// `this->f<int>()` in `D<T> : NB, T` finds the member template `NB::f`
// at the definition (a dependent base is not searched), so `<` starts a
// template argument list without `template`. At instantiation the lookup
// is done again in the complete class: when `T` has an `f` too, the name
// is ambiguous ([class.member.lookup]) and the call is an error there,
// the result differing from the definition's. A qualified `this->NB::f`
// stays valid. Clang 22 also asks for 'template' at the definition (its
// lookup into a class with a dependent base finds nothing); the text
// decides. Found running GCC's C++ testsuite (WP-1530).
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 asks for 'template' before a member template of a non-dependent base when the class has a dependent base
//@ diag: 20:26 error "member 'f' found in multiple base classes of different types"
//@ diag: 27:16 note "in instantiation of member function 'D<X>::g' requested here"
//@ diag: 18:34 note "member found by ambiguous name lookup"
//@ diag: 25:33 note "member found by ambiguous name lookup"
struct NB { template <class> int f() { return 1; } };
template <class T> struct D : NB, T {
  int g() { return this->f<int>(); }
  int h() { return this->NB::f<int>(); }
};
struct Y { int y; };
int ok = D<Y>().g() + D<Y>().h();
struct X { template <class> int f() { return 2; } };
int c = D<X>().h();
int a = D<X>().g();
