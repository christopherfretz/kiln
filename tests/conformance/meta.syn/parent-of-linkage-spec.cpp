// [dcl.link]/1,4, [meta.reflection.queries] (P2996R13 has_parent,
// parent_of, is_namespace_member, has_c_language_linkage): a linkage
// specification is transparent. A class, typedef, alias, enumeration or
// namespace declared directly in an `extern "C"` (or `extern "C++"`)
// block has no language linkage of its own, so its parent is the
// enclosing namespace (`^^::` at file scope, `^^ns` inside a namespace);
// a nested class's parent is its class. Only a function or variable
// with C language linkage (`cfn`, `bf`, `bv`) has no parent, block form
// or bare; parent_of of one is not a constant expression (GCC 16.1
// throws "reflection does not represent an entity with parent", as
// kiln: `parent-of-linkage-spec-neg.cpp`). GCC 16.1 agrees with every
// assertion.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
extern "C" {
typedef struct _cobj { int x; } CObj;
struct Q {
  struct Inner {};
  static int sm;
  void mf();
};
void bf();
extern int bv;
enum E { e1 };
using UA = int;
namespace inner_ns { struct W {}; }
}
extern "C" int cfn(int);
extern "C" struct Solo { int y; };
namespace ns {
extern "C++" { struct T {}; void tf(); }
extern "C" { struct U {}; }
}
static_assert(std::meta::has_parent(^^_cobj));
static_assert(std::meta::has_parent(^^CObj));
static_assert(std::meta::has_parent(^^Q::Inner));
static_assert(std::meta::has_parent(^^Q::sm));
static_assert(std::meta::has_parent(^^Q::mf));
static_assert(!std::meta::has_parent(^^cfn));
static_assert(!std::meta::has_parent(^^bf));
static_assert(!std::meta::has_parent(^^bv));
static_assert(std::meta::has_parent(^^E));
static_assert(std::meta::has_parent(^^e1));
static_assert(std::meta::has_parent(^^UA));
static_assert(std::meta::has_parent(^^Solo));
static_assert(std::meta::has_parent(^^ns::T));
static_assert(std::meta::has_parent(^^ns::tf));
static_assert(std::meta::has_parent(^^ns::U));
static_assert(std::meta::parent_of(^^ns::U) == ^^ns);
static_assert(std::meta::parent_of(^^e1) == ^^E);
static_assert(std::meta::parent_of(^^E) == ^^::);
static_assert(std::meta::parent_of(^^UA) == ^^::);
static_assert(std::meta::parent_of(^^inner_ns::W) == ^^inner_ns);
static_assert(std::meta::parent_of(^^inner_ns) == ^^::);
static_assert(std::meta::parent_of(^^Q::mf) == ^^Q);
static_assert(std::meta::is_namespace_member(^^_cobj));
static_assert(std::meta::is_namespace_member(^^CObj));
static_assert(!std::meta::is_namespace_member(^^Q::Inner));
static_assert(std::meta::has_identifier(^^_cobj));
static_assert(std::meta::display_string_of(^^Q::Inner) == "Q::Inner");
static_assert(std::meta::display_string_of(^^ns::U) == "ns::U");
static_assert(std::meta::is_complete_type(^^_cobj));
static_assert(std::meta::has_c_language_linkage(^^cfn));
static_assert(std::meta::has_c_language_linkage(^^bf));
static_assert(!std::meta::has_c_language_linkage(^^_cobj));
static_assert(std::meta::is_namespace_member(^^cfn));
static_assert(std::meta::parent_of(^^_cobj) == ^^::);
static_assert(std::meta::parent_of(^^CObj) == ^^::);
static_assert(std::meta::parent_of(^^Q::Inner) == ^^Q);
static_assert(std::meta::parent_of(^^ns::T) == ^^ns);
static_assert(std::meta::is_type(std::meta::parent_of(^^Q::Inner)));
static_assert(std::meta::parent_of(^^Solo) == ^^::);
