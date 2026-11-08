// [temp.variadic]/12 (P3394R4 [dcl.attr.annotation]): an annotation
// operand on a base-specifier that is not a pack expansion cannot name a
// pack: the pack is unexpanded (WP-867). Not GCC-compared: GCC 16.1
// accepts the template and crashes instantiating it (internal compiler
// error in dependent_type_p).
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:37 error "expression contains unexpanded parameter pack 'Bs'"
#include "kiln_meta.h"
struct A {};
template <class... Bs> struct D : Bs... {};
template <class... Bs> struct E : [[=sizeof(Bs)]] A {};
