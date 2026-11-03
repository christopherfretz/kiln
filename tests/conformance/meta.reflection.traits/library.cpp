// [meta.reflection.traits] P2996R13, [meta.trans.other], [refwrap.unwrapref],
// [meta.rel]: invoke_result, common_type, common_reference,
// unwrap_reference and unwrap_ref_decay have no builtin; each is the
// `::type` of the TU's std:: class template for the represented types
// (spec/reflection-traits.md §1.2), reflected canonically.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include <functional>
#include <type_traits>
#include "kiln_meta_traits.h"
using namespace std::meta;

struct B {};
struct Der : B {};
long f(int);

static_assert(invoke_result(^^decltype(f), {^^short}) == ^^long);
static_assert(invoke_result(^^int B::*, {^^B&}) == ^^int&);
static_assert(common_type({^^int, ^^long}) == ^^long);
static_assert(common_type({^^Der*, ^^B*}) == ^^B*);
static_assert(common_type({^^const int&}) == ^^int);
static_assert(common_reference({^^int&, ^^const int&}) == ^^const int&);
static_assert(common_reference({^^int&&, ^^int&&}) == ^^int&&);
static_assert(unwrap_reference(^^std::reference_wrapper<B>) == ^^B&);
static_assert(unwrap_reference(^^B) == ^^B);
static_assert(unwrap_ref_decay(^^const std::reference_wrapper<int>&) == ^^int&);
static_assert(unwrap_ref_decay(^^int[2]) == ^^int*);
// ^^std::common_type_t<int, long> would reflect the alias ([expr.reflect]).
static_assert(common_type({^^int, ^^long}) == ^^long);
