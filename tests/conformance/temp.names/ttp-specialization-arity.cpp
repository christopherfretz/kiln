// [temp.arg.general]: a template-id naming a template template parameter is
// checked against that parameter's own list ('too few/many template arguments
// for template template parameter'); a pack parameter, a default argument or a
// pack-expansion argument defers the check.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:54 error "too few template arguments for template template parameter 'TT'"
//@ diag: 17:39 note "template is declared here"
//@ diag: 18:47 error "too many template arguments for template template parameter 'TT'"
//@ diag: 18:32 note "template is declared here"
//@ diag: 19:54 error "too few template arguments for template template parameter 'TT'"
//@ diag: 19:39 note "template is declared here"
//@ diag: 20:72 error "too few template arguments for template template parameter 'TT'"
//@ diag: 20:45 note "template is declared here"
//@ diag: 23:52 error "too few template arguments for template template parameter 'TT'"
//@ diag: 23:37 note "template is declared here"
template<template<class, class> class TT> struct Q { TT<int> x; };
template<template<class> class TT> struct R { TT<int, int> x; };
template<template<class, class> class TT> void f() { TT<int> y; }
template<template<class, class = int> class TT> struct S { TT<int> ok; TT<> bad; };
template<template<class...> class TT> struct V { TT<> a; TT<int, int, int> b; };
template<template<class, class> class TT, class... Ts> struct W { TT<Ts...> c; };
template<template<int, class> class TT> struct Y { TT<1> d; };
