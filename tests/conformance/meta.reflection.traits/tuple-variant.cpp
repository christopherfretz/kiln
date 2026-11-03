// [meta.reflection.traits] P2996R13, [tuple.helper], [variant.helper]:
// tuple_size/tuple_element and variant_size/variant_alternative answer from
// the TU's std::tuple_size etc., so a user specialization counts
// ([namespace.std]/2).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include <array>
#include <tuple>
#include <utility>
#include <variant>
#include "kiln_meta_traits.h"
using namespace std::meta;

struct P { int a; };
template <>
struct std::tuple_size<P> : std::integral_constant<decltype(sizeof 0), 1> {};
template <>
struct std::tuple_element<0, P> { using type = int; };

static_assert(tuple_size(^^std::tuple<int, char, long>) == 3 && tuple_size(^^std::tuple<>) == 0);
static_assert(tuple_size(^^std::pair<int, char>) == 2 && tuple_size(^^std::array<int, 5>) == 5);
static_assert(tuple_size(^^const std::tuple<int>) == 1 && tuple_size(^^P) == 1);
static_assert(tuple_element(2, ^^std::tuple<int, char, long>) == ^^long);
static_assert(tuple_element(0, ^^const std::pair<int, char>) == ^^const int);
static_assert(tuple_element(0, ^^P) == ^^int);
static_assert(variant_size(^^std::variant<int, char>) == 2);
static_assert(variant_alternative(1, ^^std::variant<int, char>) == ^^char);
static_assert(variant_alternative(0, ^^const std::variant<int, char>) == ^^const int);
