// [meta.reflection.access.context], [meta.reflection.scope]: the
// evaluation point of access_context::current(), current_class() and
// current_function(). Inside a consteval block (nested ones too) it is in
// the scope the outermost consteval-block-declaration inhabits (/3.5),
// not in the block's own function. A default member initializer used by
// an aggregate initialization is evaluated at the point of that
// initialization (one evaluation per use, whichever use came first), and
// one used by a constructor in that constructor. Found running GCC's C++
// testsuite.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
namespace std::meta {
consteval info parent_of(info r);
}
#else
#include <meta>
#endif
using namespace std::meta;

struct V {
  consteval {
    static_assert(current_class() == ^^V);
    consteval {
      static_assert(current_class() == ^^V);
    }
  }
};

namespace N {
consteval {
  static_assert(current_namespace() == ^^N);
}
} // namespace N

void fun() {
  consteval {
    static_assert(current_function() == ^^fun);
  }
}

struct Aggr {
  access_context ctx = access_context::current();
};

constexpr Aggr first; // the implicit default constructor's use
static_assert(parent_of(first.ctx.scope()) == ^^Aggr);
constexpr Aggr at_ns{};
static_assert(at_ns.ctx.scope() == ^^::);

consteval info in_fn() {
  Aggr a{};
  return a.ctx.scope();
}
static_assert(in_fn() == ^^in_fn);

namespace M {
consteval info in_ns_fn() {
  Aggr a{};
  return a.ctx.scope();
}
} // namespace M
static_assert(M::in_ns_fn() == ^^M::in_ns_fn);
