// [dcl.attr.annotation]/1, /3 (P3394R4): a value-dependent annotation on
// a base-specifier of a class template is substituted when the class is
// instantiated; each element of a base pack expansion gets its own
// annotations, distinct from the other elements' (/3: each instantiation
// of an annotation is a unique annotation). Not GCC-compared: GCC 16.1
// crashes on the dependent operands (internal compiler error in
// dependent_type_p) and gives the pack elements equal annotations
// (spec/landed/sema/WP-830.md).
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
constexpr auto ctx = access_context::unchecked();

struct B {};
struct B2 {};
template <class T, int N> struct TD : [[=N, =sizeof(T)]] T {};
constexpr info td = bases_of(^^TD<B2, 7>, ctx)[0];
static_assert(annotations_of(td).size() == 2);
static_assert(extract<int>(annotations_of(td)[0]) == 7);
static_assert(extract<decltype(sizeof 0)>(annotations_of(td)[1]) == 1);
static_assert(annotations_of(bases_of(^^TD<B, 8>, ctx)[0])[0] != annotations_of(td)[0]);

template <int... Ns> struct PN : [[=Ns...]] B {};
static_assert(annotations_of(bases_of(^^PN<1, 2, 3>, ctx)[0]).size() == 3);
static_assert(extract<int>(annotations_of(bases_of(^^PN<1, 2, 3>, ctx)[0])[2]) == 3);

template <class... Ts> struct P : [[=5]] Ts... {};
constexpr info pb0 = bases_of(^^P<B, B2>, ctx)[0];
constexpr info pb1 = bases_of(^^P<B, B2>, ctx)[1];
static_assert(annotations_of(pb0)[0] != annotations_of(pb1)[0]);
static_assert(extract<int>(annotations_of(pb1)[0]) == 5);
