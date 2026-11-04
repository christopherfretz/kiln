// [expr.reflect]/7 (P2996R13), [class.dtor]/1, [expr.prim.id.dtor]/2: in
// `^^S::~U` the type-name after `~` must denote the class S names, and
// `^^T::~T` with T a scalar type is a pseudo-destructor name, which names
// no entity. Kiln rejects both; GCC 16.1 accepts the first and yields an
// "unsupported reflection" for the second, so this is not GCC-compared
// (spec/landed/reflection/WP-519.md).
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 17:39 error "'^^' cannot be applied to this operand"
//@ diag: 18:51 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 19:15 note "in instantiation of function template specialization 'rd<int>' requested here"
#include "kiln_meta_queries.h"
using std::meta::info;
struct S { ~S(); };
struct U {}; constexpr info a = ^^S::~U;
template <class T> consteval info rd() { return ^^T::~T; }
template info rd<int>();
