// [cpp.predefined] Table 22 (C++23, P2493R0 and later bumps): the C++20
// core feature-test macros P2493 added, with their C++23 values.
// WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
static_assert(__cpp_aggregate_paren_init >= 201902L);
static_assert(__cpp_concepts >= 202002L);
static_assert(__cpp_conditional_explicit >= 201806L);
static_assert(__cpp_consteval >= 201811L);
static_assert(__cpp_constexpr_in_decltype >= 201711L);
static_assert(__cpp_impl_coroutine >= 201902L);
static_assert(__cpp_impl_destroying_delete >= 201806L);
static_assert(__cpp_impl_three_way_comparison >= 201907L);
static_assert(__cpp_using_enum >= 201907L);
