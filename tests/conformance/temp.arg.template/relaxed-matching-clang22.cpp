// [temp.arg.template]/3-4 (P0522): packs, defaults, auto, constrained
// parameters and nested template template parameters in template template
// argument matching.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 46:22 error "template argument for template type parameter must be a type"
//@ diag: 45:40 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 46:30 note "previous template template parameter is here"
//@ diag: 55:3 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 45:48 note "previous template template parameter is here"
//@ diag: 45:33 note "template parameter is declared here"
//@ diag: 45:40 error "too many template arguments for template template parameter ''"
//@ diag: 45:40 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 47:39 note "previous template template parameter is here"
//@ diag: 56:3 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 45:48 note "previous template template parameter is here"
//@ diag: 45:40 note "template is declared here"
//@ diag: 73:25 error "template argument for template type parameter must be a type"
//@ diag: 74:4 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 73:33 note "previous template template parameter is here"
//@ diag: 72:20 note "template parameter is declared here"
//@ diag: 44:22 error "conversion from 'int' to 'int *' is not allowed in a converted constant expression"
//@ diag: 80:3 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 44:30 note "previous template template parameter is here"
//@ diag: 79:14 note "template parameter is declared here"
//@ diag: 44:22 error "non-type template parameter '' with type 'auto *' has incompatible initializer of type 'int'"
//@ diag: 82:3 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 44:30 note "previous template template parameter is here"
//@ diag: 81:15 note "template parameter is declared here"
//@ diag: 44:22 error "value of type 'int' is not implicitly convertible to 'En'"
//@ diag: 85:3 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 44:30 note "previous template template parameter is here"
//@ diag: 84:12 note "template parameter is declared here"
//@ diag: 87:13 error "use of undeclared identifier 'B'"
//@ diag: 93:3 error "too few template arguments for alias template 'AL2'"
//@ diag: 93:3 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 43:32 note "previous template template parameter is here"
//@ diag: 92:28 note "template is declared here"
//@ requires: clang>=22
// clang 22 spelling of relaxed-matching.cpp (WP-394).
template<class... T> struct V {};
template<long N> struct L {};
template<template<class> class P> struct Q {};
template<template<int> class P> struct R {};
template<template<template<class> class> class P> struct W {};
template<template<int> class X> struct WX {};
template<template<class, class> class X> struct W2 {};
template<class T, class U = int> struct D {};
template<template<class...> class P> struct S {};
template<template<class T, T> class P> struct DT {};
template<class T, T v> struct E {};
template<class T, int v> struct E2 {};
Q<V> qv;
R<L> rl;
W<WX> wx;
W<W2> w2;
Q<D> qd;
S<D> sd;
DT<E> de;
DT<E2> de2;
template<template<class> class... Ps> struct QP {};
QP<V, D> qp;
template<class> concept C = true;
template<C T> struct CC {};
Q<CC> qcc;
template<template<C> class P> struct QC {};
template<class T> struct A {};
QC<A> qca;
template<template<class> class P> struct M { P<int> x; };
template<template<class> class P> struct M2 { M<P> m; };
M2<V> m2;
template<int, class> struct N {};
template<template<int...> class P> struct NP {};
NP<N> np;
template<auto> struct AU {};
R<AU> rau;
template<template<auto> class P> struct RA {};
RA<L> ral;
template<int*> struct IP {};
R<IP> rip;
template<auto*> struct AP {};
R<AP> rap;
enum En { e0 };
template<En> struct EN {};
R<EN> ren;
template<template<class> class... Ps> struct QQ {};
QQ<A, D, V, B> qq;
template<template<class> class P> struct X1 { template<template<class> class Q> struct X2 {}; X2<P> x; };
X1<A> x1;
template<class T> using AL = A<T>;
Q<AL> qal;
template<class T, class U> using AL2 = A<T>;
Q<AL2> qal2;
