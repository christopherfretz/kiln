// [temp.arg.template]/3, [temp.constr.order]: a template template
// parameter constrained by a concept-id over a pack is at least as
// constrained as an argument with the same constraint; the pack maps to
// the parameter by position. A constraint that does not subsume the
// argument's (`true` is a different atom) rejects it.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 24:15 error "template template argument 'B' is more constrained than template template parameter 'TT'"
//@ diag: 19:56 note "'TT' declared here"
//@ diag: 18:40 note "'B' declared here"
template<class... Ts> concept C = (Ts::value && ...);
template<class... Ts> concept D = sizeof...(Ts) > 0;
template<class... Ts> requires C<Ts...> struct A1;
template<class... Ts> requires D<Ts...> struct A2;
template<class... Ts> requires C<Ts...> && D<Ts...> struct A3;
template<class T> requires true struct B;
template<template<class... Ts> requires C<Ts...> class TT> struct S;
template<template<class... Us> requires D<Us...> && C<Us...> class TT> struct S2;

using ok1 = S<A1>;
using ok2 = S2<A3>;
using bad = S<B>;
template<template<class... Ts> requires D<Ts...> class TT> struct S3 {};
S3<A2> ok3;
