// [temp.param]/10, [temp.arg.template]/1: clang 19 shape of
// ttp-default-not-class-template.cpp. Clang 19 does not parse a variable
// template or a concept as a template template argument, so they too are
// "must be a class template" at the token after the name.
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 47:47 error "default template argument for a template template parameter must be a class template"
//@ diag: 48:41 error "default template argument for a template template parameter must be a class template"
//@ diag: 49:38 error "default template argument for a template template parameter must be a class template"
//@ diag: 50:38 error "default template argument for a template template parameter must be a class template"
//@ diag: 51:47 error "default template argument for a template template parameter must be a class template"
//@ diag: 52:1 error "too few template arguments for class template 'S5'"
//@ diag: 51:56 note "template is declared here"
//@ diag: 53:71 error "default template argument for a template template parameter must be a class template"
//@ diag: 55:37 error "default template argument for a template template parameter must be a class template"
//@ diag: 56:37 error "default template argument for a template template parameter must be a class template"
//@ diag: 57:37 error "default template argument for a template template parameter must be a class template"
//@ diag: 59:37 error "default template argument for a template template parameter must be a class template"
//@ diag: 61:38 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 61:41 error "default template argument for a template template parameter must be a class template"
//@ diag: 63:38 error "default template argument for a template template parameter must be a class template"
//@ diag: 65:38 error "default template argument for a template template parameter must be a class template"

template<class> struct K {};
template<class> using A = K<int>;
template<class> int v = 0;
template<class> concept C = true;
int n;
enum E {};
namespace N { struct Y {}; template<class> struct Z {}; }

// Accepted: a class template, an alias template, a template template
// parameter, qualified and dependent template names, either key.
template<template<class> class TT = K> struct OK1 {};
template<template<class> typename TT = A> struct OK2 {};
template<template<class> class TT = ::K> struct OK3 {};
template<template<class> class TT = N::Z> struct OK4 {};
template<template<class> class TT = N::template Z> struct OK5 {};
template<template<class> class U, template<class> class TT = U> struct OK6 {};
template<class T> struct OK7 { template<template<class> class TT = T::template X> struct M; };
OK1<> ok1;
OK2<> ok2;

// Rejected: the error is at the token after the name; the parameter
// keeps no default.
template<class T, template<class> class TT = T> struct S1;
template<template<class> class TT = N::Y> struct S2;
template<template<class> class TT = n> struct S3;
template<template<class> class TT = E> struct S4;
template<template<class> class TT = undeclared> struct S5 {};
S5<> s5;
template<class T> struct S6 { template<template<class> class TT = T::X> struct M; };
// Nothing parsed: at the token itself.
template<template<class> class TT = int, class U = int> struct S7;
template<template<class> class TT = 3> struct S8;
template<template<class> class TT = (K)> struct S9;
// A template-id is one token to clang: at its start.
template<template<class> class TT = K<int>> struct S10;
// A pack expansion of a non-pack.
template<template<class> class TT = K...> struct S11;
// A variable template or a concept.
template<template<class> class TT = v> struct S12 {};
S12<K> s12;
template<template<class> class TT = C> struct S13;
