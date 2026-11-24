// [meta.reflection.names], [support.srcloc.obs], [intro.object]/9,
// [meta.define.static]: reflection's strings are objects of their own,
// never the objects of source-location strings (spec/ast.md §5.8.1), but
// strings kiln makes with equal contents keep one address, as GCC 16.1
// gives: identifier_of(^^fi) equals fi()'s __builtin_FUNCTION(), and
// identifier_of of one entity twice is one pointer. define_static_string
// of equal contents is one template parameter object
// (spec/reflection-values.md §4).
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#include <meta>

void f();
constexpr const char *fi() { return __builtin_FUNCTION(); }

static_assert(std::meta::identifier_of(^^f).data() == std::meta::identifier_of(^^f).data());
static_assert(std::meta::identifier_of(^^fi) == "fi");
consteval bool same_as_srcloc() { return std::meta::identifier_of(^^fi).data() == fi(); }
static_assert(same_as_srcloc());
static_assert(std::define_static_string("ab") == std::define_static_string("ab"));
