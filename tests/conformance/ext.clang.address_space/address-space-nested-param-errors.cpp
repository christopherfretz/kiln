// [clang ext] A parameter may not be qualified with an address space
// (clang `CheckParameter`), also one written in a function type inside
// another declarator, a typedef or a type-id: it is checked again when the
// type is substituted (clang `SubstParmVarDecl` for each parameter of the
// `FunctionProtoTypeLoc`), at the parameter. An array of qualified
// elements is rejected (the declared type is checked), a pointer or
// reference to a qualified type is not. An alias template's function type
// is substituted without its parameters, so `E<1>` is accepted, as in
// clang. Reduced from real code. spec/sema.md §5.8, WP-1823.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 41:57 error "parameter may not be qualified with an address space"
//@ diag: 42:1 note "in instantiation of template class 'A<1>' requested here"
//@ diag: 41:57 error "parameter may not be qualified with an address space"
//@ diag: 43:1 note "in instantiation of template class 'A<0>' requested here"
//@ diag: 44:58 error "parameter may not be qualified with an address space"
//@ diag: 45:1 note "in instantiation of template class 'B<1>' requested here"
//@ diag: 46:51 error "parameter may not be qualified with an address space"
//@ diag: 47:1 note "in instantiation of template class 'C<__attribute__((address_space(1))) int>' requested here"
//@ diag: 48:59 error "parameter may not be qualified with an address space"
//@ diag: 49:1 note "in instantiation of template class 'D<1>' requested here"
//@ diag: 53:45 error "parameter may not be qualified with an address space"
//@ diag: 54:6 note "in instantiation of template class 'G<1>' requested here"
//@ diag: 57:66 error "parameter may not be qualified with an address space"
//@ diag: 58:1 note "in instantiation of template class 'I<1>' requested here"
//@ diag: 59:57 error "parameter may not be qualified with an address space"
//@ diag: 60:1 note "in instantiation of template class 'J<1>' requested here"
//@ diag: 61:47 error "parameter may not be qualified with an address space"
//@ diag: 62:1 note "in instantiation of template class 'L<__attribute__((address_space(1))) int>' requested here"
//@ diag: 63:50 error "parameter may not be qualified with an address space"
//@ diag: 64:1 note "in instantiation of template class 'W<__attribute__((address_space(1))) int>' requested here"
//@ diag: 65:54 error "parameter may not be qualified with an address space"
//@ diag: 66:6 note "in instantiation of template class 'P<1>' requested here"
//@ diag: 67:54 error "parameter may not be qualified with an address space"
//@ diag: 68:13 note "in instantiation of template class 'Q<1>' requested here"
//@ diag: 69:51 error "parameter may not be qualified with an address space"
//@ diag: 70:1 note "in instantiation of template class 'R<__attribute__((address_space(1))) int[2]>' requested here"
//@ diag: 73:54 error "parameter may not be qualified with an address space"
//@ diag: 74:1 note "in instantiation of template class 'X<__attribute__((address_space(1))) int>' requested here"
#define AS(n) __attribute__((address_space(n)))
template <int N> struct A { using t = void (*)(int AS(N)); };
A<1>::t a1;
A<0>::t a0;
template <int N> struct B { using t = void (*)(int AS(N) p); };
B<1>::t b1;
template <class T> struct C { using t = void (*)(T); };
C<int __attribute__((address_space(1)))>::t c1;
template <int N> struct D { using t = int (D::*)(int AS(N)); };
D<1>::t d1;
template <int N> using E = void (*)(int AS(N));
E<1> e1;
template <class T> struct F {};
template <int N> struct G { F<void(int AS(N))> f; };
G<1> g1;
template <int N> struct H { using t = void (*)(int AS(N) *); };
H<1>::t h1;
template <int N> struct I { using t = void (*)(void (*)(int AS(N))); };
I<1>::t i1;
template <int N> struct J { using t = void (*)(int AS(N)[]); };
J<1>::t j1;
template <class T> struct L { using t = void(T); };
L<int __attribute__((address_space(1)))>::t *l1;
template <class T> struct W { typedef void (*t)(T); };
W<int __attribute__((address_space(1)))>::t w1;
template <int N> struct P { void f(void (*)(int AS(N))); };
P<1> p1;
template <int N> struct Q { void f(void (*)(int AS(N))) {} };
void uq() { Q<1>().f(nullptr); }
template <class T> struct R { using t = void (*)(T); };
R<int __attribute__((address_space(1)))[2]>::t r1;
template <int N> struct U { using t = void (*)(int AS(N) &); };
U<1>::t u1;
template <class T> struct X { using t = decltype([](T) {}); };
X<int __attribute__((address_space(1)))>::t *x1;
