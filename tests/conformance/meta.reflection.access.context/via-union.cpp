// [meta.reflection.access.context] (P2996R13), [class.pre]/1: via(cls)
// needs a null reflection or a complete class type, and a union is a
// class, so via(^^U) designates U and is_accessible checks U's members
// through it. GCC 16.1's <meta> rejects a union (its `via` tests
// is_class_type, which excludes unions), so this follows the draft and is
// not GCC-compared (spec/reflection-members.md §3, spec/landed/sema/WP-830.md).
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ xfail(gcc): GCC 16.1's access_context::via rejects a union (its is_class_type check excludes unions)
//@ meta-header: skip GCC 16.1's <meta> access_context::via rejects a union (as xfail(gcc))
#include "kiln_meta.h"
using namespace std::meta;

union U {
  int pub;
 private:
  int priv;
  friend consteval bool peek();
};
constexpr access_context ctx = access_context::unprivileged().via(^^U);
static_assert(ctx.designating_class() == ^^U);
static_assert(is_accessible(^^U::pub, ctx));
consteval bool peek() {
  return is_accessible(nonstatic_data_members_of(^^U, access_context::unchecked())[1],
                       access_context::current().via(^^U));
}
static_assert(peek());
static_assert(!is_accessible(nonstatic_data_members_of(^^U, access_context::unchecked())[1], ctx));
