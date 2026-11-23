// [meta.reflection.queries], [basic.link]/4, [dcl.link]/4: a function or
// variable of an unnamed namespace first declared in an `extern "C"`
// context (a block-scope `extern` one in an `extern "C"` function of an
// unnamed namespace included) has external linkage and C language
// linkage, as clang gives it and as its symbol is emitted (an unmangled
// external name, one entity with a global `extern "C"` declaration of the
// same name); one outside an `extern "C"` context is internal and has no
// language linkage.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ xfail(gcc): GCC 16.1 gives an `extern "C"` name in an unnamed namespace internal linkage (and C language linkage)
#include "kiln_meta_queries.h"
using namespace std::meta;

namespace {
extern "C" void f();
extern "C" int x;
void plain();
extern "C" void k() {
  extern int w;
  extern void m();
  static_assert(has_external_linkage(^^w) && has_c_language_linkage(^^w));
  static_assert(has_external_linkage(^^m) && has_c_language_linkage(^^m));
}
} // namespace

static_assert(has_external_linkage(^^f) && !has_internal_linkage(^^f));
static_assert(has_external_linkage(^^x) && has_c_language_linkage(^^x));
static_assert(has_c_language_linkage(^^f) && has_c_language_linkage(^^k));
static_assert(has_internal_linkage(^^plain) && !has_c_language_linkage(^^plain));
