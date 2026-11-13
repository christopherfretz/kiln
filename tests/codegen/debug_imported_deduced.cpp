// Debug info (WP-1389, [namespace.udecl], [dcl.spec.auto.general]): a
// using-declaration naming a function whose return type is deduced from a
// definition after it. clang hands a whole top-level declaration (a
// namespace, a linkage specification) to codegen once parsed, and emits a
// namespace-scope using-declaration then; a block-scope one when its
// function is emitted: at the end of the function's top-level declaration,
// or at the end of the unit for an inline function. The return type is
// deduced by then, and the entity emitted, when the definition is in that
// top-level declaration or an earlier one; otherwise nothing is emitted.
//@ args: -g
namespace A {
namespace N {
auto f();
}
using N::f; // defined later in the same top-level namespace: emitted
namespace N {
auto f() { return 1; }
} // namespace N
} // namespace A

namespace N {
auto g();
auto h() { return 3; }
auto p();
auto q();
auto r();
} // namespace N
using N::g; // defined in a later top-level declaration: nothing
using N::h; // defined before: emitted
namespace N {
auto g() { return 2; }
} // namespace N

extern "C++" {
namespace G {
auto u();
}
using G::u; // the linkage specification is one top-level declaration
namespace G {
auto u() { return 4; }
} // namespace G
}

namespace B {
int use1() {
  using N::p; // defined in a later top-level declaration: nothing
  return 0;
}
namespace N {
auto t();
}
int use2() {
  using N::t; // use2 is emitted at the end of B: emitted
  return 1;
}
namespace N {
auto t() { return 5; }
} // namespace N
} // namespace B

namespace N {
auto p() { return 6; }
} // namespace N

inline int use3() {
  using N::q; // an inline body is emitted at the end of the unit: emitted
  return 2;
}
int (*pu3)() = use3;

int main() {
  struct L {
    static int m() {
      using N::r; // a local class's member is inline: emitted
      return 3;
    }
  };
  return L::m() + B::use1() + B::use2();
}

namespace N {
auto q() { return 7; }
auto r() { return 8; }
} // namespace N
