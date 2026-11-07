// [temp.spec.partial.general]/9.3, [temp.over.link]/6, [temp.deduct.type]/20:
// a partial specialization of a template whose non-type parameter is
// `auto` may repeat that parameter with a more specific type
// (`T *P`, `T C::*M`, `int N`); its argument list then differs from the
// primary's, and the parameters that occur only in that type (`T`, `C`)
// are deduced from the type of the argument. The intrusive queue and heap
// shapes are those of a sender/receiver library (WP-789).
//@ expect: run
//@ std: c++20
//@ stdout: 1 2 3 3 3 1 | 1 2 | 8 4
extern "C" int printf(const char *, ...);

template <auto V> struct S { static constexpr int k = 0; };
template <class T, T *P> struct S<P> { static constexpr int k = 1; };
template <class T, class C, T C::*M> struct S<M> { static constexpr int k = 2; };
template <int N> struct S<N> { static constexpr int k = 3; };

struct A { int x; double y; };
int g;
long h;

template <class T> struct box { T v; };
template <auto Next> class queue;
template <class Node, box<Node *> Node::*Next>
  requires requires { Node{}; }
class queue<Next> {
public:
  Node stub{};
  int f() { return (stub.*Next).v == nullptr ? 1 : 0; }
};
struct qnode { box<qnode *> next; };

template <class Node, class KeyT, auto Key, auto Prev> class heap;
template <class Node, class KeyT, KeyT Node::*Key, Node *Node::*Prev>
class heap<Node, KeyT, Key, Prev> {
public:
  int g() { return sizeof(KeyT) + 0 * sizeof(Node); }
};
struct hnode { long key; int key2; hnode *prev; };

int main() {
  printf("%d %d %d %d %d %d | ", S<&g>::k, S<&A::y>::k, S<3>::k, S<'c'>::k, S<3L>::k,
         S<&h>::k);
  printf("%d %d | ", queue<&qnode::next>{}.f(), S<&qnode::next>::k);
  printf("%d %d\n", heap<hnode, long, &hnode::key, &hnode::prev>{}.g(),
         heap<hnode, int, &hnode::key2, &hnode::prev>{}.g());
}
