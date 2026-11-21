// [clang ext] [temp.deduct.general]/8 The Swift parameter ABIs of a
// function template's own type and of a function type in its trailing
// return type are checked again after substitution (clang
// `BuildFunctionType` → `checkExtParameterInfos`): an empty pack that
// leaves a `swift_error_result` without the `swift_context` before it, or
// a `swift_context` parameter that is not a pointer, is a substitution
// failure in the immediate context, so the requirement is not satisfied
// (reduced from real code). spec/sema.md §4.5, WP-1839.
//@ expect: compile-ok
//@ strict-diags
template <class... T>
using P2 = __attribute__((swiftcall)) void (*)(T __attribute__((swift_context))...,
                                               void **__attribute__((swift_error_result)));
template <class... T> auto h2(T...) -> P2<T...>;
template <class... X> concept CH2 = requires { h2((X)0 ...); };
static_assert(!CH2<>);
static_assert(CH2<void *>);
template <class... T>
__attribute__((swiftcall)) void g3(T __attribute__((swift_context))...,
                                   void **__attribute__((swift_error_result)));
template <class... X> concept CG3 = requires { g3<X...>((X)0 ..., nullptr); };
static_assert(!CG3<>);
static_assert(CG3<void *>);
template <class... T>
auto h3() -> __attribute__((swiftcall)) void (*)(T __attribute__((swift_context))...,
                                                 void **__attribute__((swift_error_result)));
template <class... X> concept CH3 = requires { h3<X...>(); };
static_assert(!CH3<>);
static_assert(CH3<void *>);
template <class T> auto h4() -> __attribute__((swiftcall)) void (*)(T __attribute__((swift_context)));
template <class X> concept CH4 = requires { h4<X>(); };
static_assert(!CH4<int>);
static_assert(CH4<int *>);
