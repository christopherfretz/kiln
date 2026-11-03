// [meta.reflection.access.queries] (P2996R13): has_inaccessible_bases,
// has_inaccessible_nonstatic_data_members and has_inaccessible_subobjects
// (the union of both) ask is_accessible of each direct base or non-static
// data member of a complete class.
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

constexpr auto here = access_context::current();
constexpr auto all = access_context::unchecked();

struct B {};
struct Pub : B {
  int a;
};
class PrivBase : B {
 public:
  int a;
  static consteval bool inside() { return has_inaccessible_subobjects(^^PrivBase, access_context::current()); }
};
class PrivMember : public B {
  int a;
  static int s;  // not a non-static data member
};
struct Empty {};

static_assert(!has_inaccessible_bases(^^Pub, here));
static_assert(!has_inaccessible_nonstatic_data_members(^^Pub, here));
static_assert(!has_inaccessible_subobjects(^^Pub, here));

static_assert(has_inaccessible_bases(^^PrivBase, here));
static_assert(!has_inaccessible_nonstatic_data_members(^^PrivBase, here));
static_assert(has_inaccessible_subobjects(^^PrivBase, here));
static_assert(!has_inaccessible_subobjects(^^PrivBase, all));
static_assert(!PrivBase::inside());

static_assert(!has_inaccessible_bases(^^PrivMember, here));
static_assert(has_inaccessible_nonstatic_data_members(^^PrivMember, here));
static_assert(has_inaccessible_subobjects(^^PrivMember, access_context::unprivileged()));
static_assert(!has_inaccessible_nonstatic_data_members(^^PrivMember, all));

static_assert(!has_inaccessible_subobjects(^^Empty, here));

int main() {}
