// [meta.reflection.traits] P2996R13, [meta.unary.prop]: the type property traits
// (is_const_type ... is_scoped_enum_type), delegated to the builtin trait engine. A
// complete type is required where [meta.unary.prop] requires one (see failures-neg.cpp).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ meta-header: skip GCC 16.1 declares is_structural_type only with __builtin_is_structural (follow-up)
#include "kiln_meta_traits.h"
using namespace std::meta;

struct Empty {};
struct Poly { virtual void f(); };
struct Abs { virtual void f() = 0; };
struct Fin final {};
struct Agg { int a; double b; };
struct NotAgg { NotAgg(int); };
struct Lit { int x; };
struct Priv { private: int x; public: constexpr Priv() : x() {} };
struct Mixed { int a; private: int b; };
enum class SE : unsigned char { a };
enum UE { u };

static_assert(is_const_type(^^const int) && !is_const_type(^^const int&));
static_assert(is_volatile_type(^^volatile int) && !is_volatile_type(^^int));
static_assert(is_trivially_copyable_type(^^Agg) && !is_trivially_copyable_type(^^Poly));
static_assert(is_standard_layout_type(^^Agg) && !is_standard_layout_type(^^Mixed));
static_assert(is_empty_type(^^Empty) && !is_empty_type(^^Agg) && !is_empty_type(^^int));
static_assert(is_polymorphic_type(^^Poly) && !is_polymorphic_type(^^Agg));
static_assert(is_abstract_type(^^Abs) && !is_abstract_type(^^Poly));
static_assert(is_final_type(^^Fin) && !is_final_type(^^Empty));
static_assert(is_aggregate_type(^^Agg) && is_aggregate_type(^^int[2]) && !is_aggregate_type(^^NotAgg));
static_assert(is_signed_type(^^int) && !is_signed_type(^^unsigned) && !is_signed_type(^^Agg));
static_assert(is_unsigned_type(^^unsigned) && is_unsigned_type(^^bool) && !is_unsigned_type(^^SE));
static_assert(is_bounded_array_type(^^int[2]) && !is_bounded_array_type(^^int[]));
static_assert(is_unbounded_array_type(^^int[]) && !is_unbounded_array_type(^^int[2]));
static_assert(is_scoped_enum_type(^^SE) && !is_scoped_enum_type(^^UE));
static_assert(has_virtual_destructor(^^Poly) == __has_virtual_destructor(Poly));
static_assert(has_unique_object_representations(^^int));
static_assert(!has_unique_object_representations(^^float));

// is_structural_type ([temp.param]/7): a scalar, an lvalue reference, or a
// literal class whose bases and non-static data members are public and
// non-mutable structural types. std::meta::info is scalar, so structural.
static_assert(is_structural_type(^^int) && is_structural_type(^^int&));
static_assert(is_structural_type(^^info) && is_structural_type(^^Lit));
static_assert(!is_structural_type(^^Priv) && !is_structural_type(^^NotAgg));
static_assert(!is_structural_type(^^int&&));

static_assert(is_trivially_copyable_type(^^Agg) == __is_trivially_copyable(Agg));
static_assert(is_aggregate_type(^^NotAgg) == __is_aggregate(NotAgg));
