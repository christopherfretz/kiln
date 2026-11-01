// [temp.arg.template]/3-4 (P0522): packs, defaults, auto, constrained
// parameters and nested template template parameters in template template
// argument matching.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:3 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 30:22 note "template parameter has a different kind in template argument"
//@ diag: 29:33 note "previous template template parameter is here"
//@ diag: 40:3 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 31:10 note "too many template parameters in template template argument"
//@ diag: 29:19 note "previous template template parameter is here"
//@ diag: 58:4 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 56:20 note "template parameter has a different kind in template argument"
//@ diag: 57:25 note "previous template template parameter is here"
//@ diag: 64:3 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 63:14 note "template non-type parameter has a different type 'int *' in template argument"
//@ diag: 28:22 note "previous non-type template parameter with type 'int' is here"
//@ diag: 69:3 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 68:12 note "template non-type parameter has a different type 'En' in template argument"
//@ diag: 28:22 note "previous non-type template parameter with type 'int' is here"
//@ diag: 71:13 error "use of undeclared identifier 'B'"
//@ diag: 77:3 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 76:1 note "too many template parameters in template template argument"
//@ diag: 27:10 note "previous template template parameter is here"
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
