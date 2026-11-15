// [meta.reflection.queries] has_parent, parent_of, is_class_member,
// has_c_language_linkage, has_linkage; [basic.link], [dcl.link]/2,5,
// [basic.start.main]/3, [class.union.anon]/1, [dcl.pre] (consteval
// blocks). Cases where GCC 16.1 answers otherwise, kiln following the
// working draft (docs/status/deviations.md):
// - `main` has C++ language linkage (it is not declared in a linkage
//   specification), so it has a parent; GCC gives it C language linkage.
// - A function with internal linkage declared in `extern "C"` has no
//   language linkage of its name ([dcl.link]/5 gives C linkage only to
//   function types and to names with external linkage).
// - An alias template is a template, so it has the linkage of its
//   namespace ([basic.link]/4); GCC says it has none.
// - A local of a consteval block is a local of the block's closure call
//   operator ([dcl.pre]: the block is evaluated as `[] -> void consteval
//   compound-statement ()`); GCC answers the enclosing namespace.
// Agreeing with GCC: a member of a namespace-scope anonymous union is a
// class member, not a namespace member, and members_of the namespace
// lists the union's unnamed variable and its class. Found running GCC's C++
// testsuite.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ xfail(gcc): GCC 16.1 follows its own language linkage of main and of internal functions in extern "C", gives alias templates no linkage, and makes consteval blocks transparent to parent_of
//@ expect: compile-ok
//@ strict-diags
#ifdef __KILN__
#include "kiln_meta.h"
#include "kiln_meta_queries.h"
#include "kiln_meta_traits.h"
#else
#include <meta>
#endif
using namespace std::meta;

int main() {}
static_assert(has_parent(^^main) && parent_of(^^main) == ^^::);
static_assert(!has_c_language_linkage(^^main));

extern "C" {
static void internal_c() {}
void external_c();
}
static_assert(!has_c_language_linkage(^^internal_c) && has_internal_linkage(^^internal_c));
static_assert(has_c_language_linkage(^^external_c) && !has_parent(^^external_c));

template <class T> using Alias = T*;
static_assert(has_linkage(^^Alias) && has_external_linkage(^^Alias));

consteval {
  int local = 0;
  static_assert(is_function(parent_of(^^local)));
  static_assert(parent_of(^^local) != ^^::);
}

static union {
  int in_union;
};
static_assert(is_class_member(^^in_union) && !is_namespace_member(^^in_union));
static_assert(is_union_type(parent_of(^^in_union)));

// The anonymous union's unnamed variable and its class are both members
// of the namespace (agreeing with GCC).
namespace with_union {
static union {
  int m;
};
}
consteval bool union_members() {
  auto v = members_of(^^with_union, access_context::current());
  return v.size() == 2 && is_variable(v[0]) != is_variable(v[1]) &&
         !has_identifier(v[0]) && !has_identifier(v[1]);
}
static_assert(union_members());
