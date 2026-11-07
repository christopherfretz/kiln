// [temp.deduct.call]/6, [over.over]/3, [temp.arg.explicit]/4: the
// argument `g.Is<3>` is an overloaded member access, a template-id
// naming the one specialization `G::Is<3>`, a static member function;
// for a parameter `P` it gives `A` = `bool(int)`, decayed to
// `bool (*)(int)`, and a function pointer parameter is deduced the same
// way. The conversion names the static member without "cannot create a
// non-constant pointer to member function" (a template-id is resolved
// first, clang's `ResolveSingleFunctionTemplateSpecialization`). LLVM's
// `members_if(DFG.IsRef<NodeAttrs::Use>, DFG)` (RDFLiveness.cpp).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 1 0 1 1
#include <cstdio>

struct G {
  template <int K> static bool Is(int x) { return x == K; }
  bool Is(long) const { return false; }
};
struct N {
  template <class P> int count(P p, const G &) const { return p(3) ? 1 : 0; }
  template <class R> int via_ptr(R (*p)(int)) const { return p(4) ? 1 : 0; }
};

int main() {
  G g;
  N n;
  bool (*fp)(int) = g.Is<4>;
  std::printf("%d %d %d %d\n", n.count(g.Is<3>, g), n.count(g.Is<2>, g), n.via_ptr(g.Is<4>),
              fp(4));
}
