// [dcl.attr.annotation]/1, [meta.reflection.annotation] (P3394R4): an
// annotation may appertain to a base-specifier; annotations_of of the
// direct base class relationship bases_of gives lists them in source
// order, each of the type of its underlying constant. A class template
// specialization's base-specifier has its pattern's annotations.
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;
constexpr auto ctx = access_context::unchecked();

struct B {};
struct B2 {};
struct D : [[=1]] [[=2, ='c']] B, B2 {};
constexpr info b0 = bases_of(^^D, ctx)[0];
constexpr info b1 = bases_of(^^D, ctx)[1];
static_assert(annotations_of(b0).size() == 3);
static_assert(annotations_of(b1).size() == 0);
static_assert(extract<int>(annotations_of(b0)[1]) == 2);
static_assert(extract<char>(annotations_of(b0)[2]) == 'c');
static_assert(type_of(annotations_of(b0)[0]) == ^^int);
static_assert(annotations_of_with_type(b0, ^^int).size() == 2);
static_assert(annotations_of_with_type(b0, ^^char).size() == 1);
static_assert(is_annotation(annotations_of(b0)[0]));

struct V : [[=3]] virtual public B {};
static_assert(extract<int>(annotations_of(bases_of(^^V, ctx)[0])[0]) == 3);

template <class T> struct Q : [[=4]] T {};
static_assert(annotations_of(bases_of(^^Q<B>, ctx)[0]).size() == 1);
static_assert(extract<int>(annotations_of(bases_of(^^Q<B>, ctx)[0])[0]) == 4);

template <class... Ts> struct P : [[=5]] Ts... {};
static_assert(annotations_of(bases_of(^^P<B, B2>, ctx)[1]).size() == 1);
