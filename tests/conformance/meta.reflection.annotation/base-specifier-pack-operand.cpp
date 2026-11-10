// [temp.variadic]/5.4, [dcl.attr.annotation]/3 (P3394R4): the pattern of a
// base-specifier pack expansion is the whole base-specifier, its
// attribute-specifier-seq included, so an annotation operand may name the
// pack and is substituted for each base the expansion gives; operands
// that do not name it are substituted once, in source order with the
// others. Not GCC-compared: GCC 16.1 crashes on the operand naming the
// pack (internal compiler error in dependent_type_p; docs/status/
// deviations.md "GCC 16.1 reflection divergences"; WP-867).
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ xfail(gcc): GCC 16.1 crashes (internal compiler error in dependent_type_p)
#include "kiln_meta.h"
using namespace std::meta;
constexpr auto ctx = access_context::unchecked();

struct A { char c; };
struct B { int i[3]; };
template <class... Bs> struct D : [[=sizeof(Bs)]] Bs... {};
static_assert(bases_of(^^D<A, B>, ctx).size() == 2);
static_assert(extract<decltype(sizeof 0)>(annotations_of(bases_of(^^D<A, B>, ctx)[0])[0]) == sizeof(A));
static_assert(extract<decltype(sizeof 0)>(annotations_of(bases_of(^^D<A, B>, ctx)[1])[0]) == sizeof(B));
static_assert(bases_of(^^D<>, ctx).size() == 0);

template <int N, class... Bs> struct E : [[=N, =sizeof(Bs) * 2, =3]] Bs... {};
constexpr info e1 = bases_of(^^E<9, A, B>, ctx)[1];
static_assert(annotations_of(e1).size() == 3);
static_assert(extract<int>(annotations_of(e1)[0]) == 9);
static_assert(extract<decltype(sizeof 0)>(annotations_of(e1)[1]) == 2 * sizeof(B));
static_assert(extract<int>(annotations_of(e1)[2]) == 3);
static_assert(extract<decltype(sizeof 0)>(annotations_of(bases_of(^^E<9, A, B>, ctx)[0])[1]) == 2 * sizeof(A));
static_assert(annotations_of(bases_of(^^E<9, A, B>, ctx)[0])[0] != annotations_of(e1)[0]);

template <class T> struct Tag { static constexpr int v = sizeof(T); };
template <class... Bs> struct F : [[=Tag<Bs>::v]] virtual Bs... {};
static_assert(extract<int>(annotations_of(bases_of(^^F<A, B>, ctx)[1])[0]) == sizeof(B));
