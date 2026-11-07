// [expr.ref]/4, [class.mem]/3: in `d.E::e` with `E` an unscoped
// enumeration of a base class, `e` is a member of that base, so the
// access is valid; the enumeration names no class, so only the name `E`
// is access-checked (clang's `CheckQualifiedMemberReference` uses the
// member's non-transparent context). LLVM's
// `TLI.ReciprocalEstimate::Disabled` (DAGCombiner.cpp). Verified against
// clang 19.
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 3 1 4 5
#include <cstdio>
struct B {
  enum E : int { Disabled = -1, On = 1 };
};
struct D : B {};
class C {
  enum P { Hidden = 4 };
public:
  enum Q { Shown = 5 };
};
struct DC : C {};
int f(const D &d, int x) {
  int r = 0;
  if (x == d.E::Disabled) r += 1;
  if (x == d.B::E::On) r += 2;
  return r;
}
template <class T> int g(const T &t) { return t.E::On; }
int main() {
  D d;
  DC c;
  std::printf("%d %d %d %d\n", f(d, -1) + f(d, 1), g(d), (int)C::Q::Shown - 1,
              c.Q::Shown);
}
