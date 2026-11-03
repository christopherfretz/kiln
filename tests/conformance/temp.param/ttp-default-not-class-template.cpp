// [temp.param]/10, [temp.arg.template]/1: the default argument of a template
// template parameter must name a class template, an alias template or a
// template template parameter as a whole template argument; anything else is
// "default template argument for a template template parameter must be a
// class template" at the token after what was parsed (clang's
// ParseTemplateTemplateArgument). Clang 22 parses a variable template or
// a concept as a template name (P2841) and rejects it as an argument of the
// wrong kind; the clang 19 shape is ttp-default-not-class-template-clang19.cpp.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 53:47 error "default template argument for a template template parameter must be a class template"
//@ diag: 54:41 error "default template argument for a template template parameter must be a class template"
//@ diag: 55:38 error "default template argument for a template template parameter must be a class template"
//@ diag: 56:38 error "default template argument for a template template parameter must be a class template"
//@ diag: 57:47 error "default template argument for a template template parameter must be a class template"
//@ diag: 58:1 error "too few template arguments for class template 'S5'"
//@ diag: 57:56 note "template is declared here"
//@ diag: 59:71 error "default template argument for a template template parameter must be a class template"
//@ diag: 61:37 error "default template argument for a template template parameter must be a class template"
//@ diag: 62:37 error "default template argument for a template template parameter must be a class template"
//@ diag: 63:37 error "default template argument for a template template parameter must be a class template"
//@ diag: 65:37 error "default template argument for a template template parameter must be a class template"
//@ diag: 67:38 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 67:41 error "default template argument for a template template parameter must be a class template"
//@ diag: 69:37 error "template argument does not refer to a class or alias template, or template template parameter"
//@ diag: 33:21 note "template argument refers to a variable template 'v', here"
//@ diag: 71:37 error "template argument does not refer to a class or alias template, or template template parameter"
//@ diag: 34:25 note "template argument refers to a concept 'C', here"

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
