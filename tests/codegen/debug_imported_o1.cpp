// Debug info (WP-1365, [namespace.udecl], [namespace.udir]): when
// optimizing, a subprogram's `retainedNodes` keep its parameters and
// variables and its block-scope imported entities in creation order.
//@ args: -g -O1 -Xclang -disable-llvm-passes
namespace N {
int v = 1;
struct P {
  int q;
};
} // namespace N
int caller(int a) {
  int local = a;
  using N::P;
  P p{local};
  {
    using namespace N;
    int inner = v;
    local += inner;
  }
  return p.q + local;
}
