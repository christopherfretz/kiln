// [meta.reflection.annotation] (P3394R4): annotations_of(r) is constant
// when r represents a namespace, which includes a namespace alias
// ([meta.reflection.queries]: is_namespace is true of both). A namespace
// alias has its own annotations, and a namespace-alias-definition
// ([namespace.alias]) admits none, so annotations_of and
// annotations_of_with_type of an alias are empty: the aliased namespace's
// annotations are not seen through it. GCC 16.1 agrees.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta_queries.h"
#else
#include <meta>
#endif
using namespace std::meta;

struct skip_t {};

namespace [[=1]] inner {}
namespace alias = inner;
namespace alias2 = alias;

static_assert(annotations_of(^^inner).size() == 1);
static_assert(annotations_of(^^alias).size() == 0);
static_assert(annotations_of(^^alias2).size() == 0);
static_assert(annotations_of_with_type(^^inner, ^^int).size() == 1);
static_assert(annotations_of_with_type(^^alias, ^^int).size() == 0);
static_assert(annotations_of_with_type(^^alias2, ^^int).size() == 0);
static_assert(annotations_of_with_type(^^alias, ^^skip_t).size() == 0);

static_assert(is_namespace(^^alias) && is_namespace_alias(^^alias));
static_assert(is_namespace(^^alias2) && is_namespace_alias(^^alias2));
static_assert(is_namespace(^^inner) && !is_namespace_alias(^^inner));

// A namespace whose members include a function, a variable, a nested
// namespace and a namespace alias: every member is a valid argument.
namespace ns {
  [[=skip_t{}]] void f();
  int v;
  namespace [[=skip_t{}]] nested {}
  namespace nested_alias = nested;
  namespace outer_alias = ::inner;
}

consteval int count_skipped(info n) {
  int k = 0;
  for (info m : members_of(n, access_context::unchecked()))
    k += annotations_of_with_type(m, ^^skip_t).size();
  return k;
}
static_assert(count_skipped(^^ns) == 2);

consteval int count_aliases(info n) {
  int k = 0;
  for (info m : members_of(n, access_context::unchecked()))
    if (is_namespace_alias(m)) {
      if (!is_namespace(m) || annotations_of(m).size() != 0) return -1;
      ++k;
    }
  return k;
}
static_assert(count_aliases(^^ns) == 2);

static_assert(annotations_of(^^ns::nested).size() == 1);
static_assert(annotations_of(^^ns::nested_alias).size() == 0);
static_assert(annotations_of(^^ns::outer_alias).size() == 0);
