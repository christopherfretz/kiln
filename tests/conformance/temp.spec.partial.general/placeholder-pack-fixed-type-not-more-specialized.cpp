// [temp.spec.partial.general]/9.2, [temp.deduct.type]/20: the primary's
// `auto...` element synthesized from a partial specialization's `int...`
// pack has a unique placeholder type, which `int` does not match, so the
// partial is not more specialized than the primary (clang 22, WP-1105).
// Clang 19 accepted it; -fclang-compat=19 keeps that
// (placeholder-pack-fixed-type-clang19.cpp).
//@ std: c++20
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:29 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 15:30 note "template is declared here"
//@ diag: 18:30 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 17:30 note "template is declared here"
template <auto... Vs> struct S { static constexpr int v = 0; };
template <int... Is> struct S<Is...> { static constexpr int v = 1; };
template <auto... Vs> struct P { static constexpr int v = 0; };
template <int *...Ps> struct P<Ps...> { static constexpr int v = 1; };
