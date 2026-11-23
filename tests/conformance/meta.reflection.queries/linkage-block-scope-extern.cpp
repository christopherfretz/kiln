// [meta.reflection.queries], [basic.link]/4, [basic.link]/6,
// [dcl.meaning.general]/3.5: a block-scope function declaration or
// `extern` variable declares a member of the innermost enclosing
// namespace, so has_internal_linkage holds for one in a function (or a
// member function) of an unnamed namespace, has_external_linkage for one
// in a `static` function outside one, and one redeclaring an earlier
// `static` variable takes its internal linkage. A local variable has
// none. Answers checked against GCC 16.1 (-freflection).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta_queries.h"
using namespace std::meta;

namespace {
void g() {
  extern int v;
  extern void h();
  int loc = 0;
  (void)loc;
  static_assert(has_internal_linkage(^^v) && !has_external_linkage(^^v));
  static_assert(has_internal_linkage(^^h) && has_linkage(^^h));
  static_assert(!has_c_language_linkage(^^v) && !has_linkage(^^loc));
}
struct A {
  void m() {
    extern int mv;
    static_assert(has_internal_linkage(^^mv));
  }
};
} // namespace

namespace N {
namespace {
void q() {
  extern void nh();
  static_assert(has_internal_linkage(^^nh));
}
} // namespace
} // namespace N

static void s() {
  extern int sv;
  extern void sh();
  static_assert(has_external_linkage(^^sv) && has_external_linkage(^^sh));
  static_assert(!has_internal_linkage(^^sv) && !has_c_language_linkage(^^sh));
}

static int pv;
void p() {
  extern int pv;
  static_assert(has_internal_linkage(^^pv));
}
