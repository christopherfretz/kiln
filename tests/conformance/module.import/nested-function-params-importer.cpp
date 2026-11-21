// [module.import]/2, [clang ext] [temp.deduct.general]/8 (WP-1859): the
// parameters of a function type written inside a declarator of an
// imported function template (a parameter's type, a trailing return type)
// are checked again when the importer substitutes the template, as in the
// module itself (address-space-nested-param-sfinae.cpp,
// nested-instantiation-sfinae.cpp): an address space on such a parameter
// and a Swift parameter ABI on one of the wrong type or position are
// substitution failures in the immediate context, so the requirements
// naming those specializations are not satisfied. An alias template's
// function type is substituted without its parameters.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ module: nfp=Inputs/nested-function-params.cppm
import nfp;
#define AS(n) __attribute__((address_space(n)))

template <int N> concept CA1 = requires { a1<N>(); };
static_assert(!CA1<1>);
template <class X> concept CA2 = requires { a2<X>(nullptr); };
static_assert(!CA2<int AS(1)>);
static_assert(CA2<int>);
static_assert(CA2<int AS(1) *>);
template <class X> concept CA3 = requires { a3<X>(nullptr); };
static_assert(!CA3<int AS(1)>);
static_assert(CA3<int>);
template <class X> concept CE = requires { typename E<X>; };
static_assert(CE<int AS(1)>);
template <class... X> concept CH3 = requires { h3<X...>(); };
static_assert(!CH3<>);
static_assert(CH3<void *>);
template <class X> concept CH4 = requires { h4<X>(); };
static_assert(!CH4<int>);
static_assert(CH4<int *>);
