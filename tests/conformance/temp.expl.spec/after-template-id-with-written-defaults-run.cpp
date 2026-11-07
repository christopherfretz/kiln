// [temp.inst]/2, [temp.expl.spec]/7: naming `It<N*, Set<N*>, false,
// GT<N*>>`, every argument written, does not instantiate `GT<N*>`, even
// though the default of the second parameter is
// `Set<typename GT<G>::NodeRef>`. The compiler's check whether a written
// argument equals its default (used only to print the specialization's
// name) must not substitute that default, so the explicit specialization
// of `GT<N*>` declared afterwards is not "after instantiation" and the
// primary's hard error is never reached. LLVM's
// `RegionBase::element_iterator` before `GraphTraits<RegionNode *>`
// (RegionInfo.h, RegionIterator.h). Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 5 5
#include <cstdio>

template <class G> struct GT {
  using NodeRef = typename G::UnknownGraphTypeError;
};
template <class N> struct Set {};
template <class G, class S = Set<typename GT<G>::NodeRef>, bool E = false,
          class T = GT<G>>
class It {
public:
  using value_type = typename T::NodeRef;
  value_type v;
};

template <class Tr> struct RB {
  using NodeT = typename Tr::NodeT;
  using element_iterator = It<NodeT *, Set<NodeT *>, false, GT<NodeT *>>;
  element_iterator first() const;
};

struct Node;
struct Traits {
  using NodeT = Node;
};
struct Region : RB<Traits> {};

template <> struct GT<Node *> {
  using NodeRef = Node *;
};
struct Node {
  int x;
};

template <class Tr>
typename RB<Tr>::element_iterator RB<Tr>::first() const {
  static Node n{5};
  return element_iterator{&n};
}

int main() {
  Region r;
  It<Node *> d{r.first().v};
  std::printf("%d %d\n", r.first().v->x, d.v->x);
}
