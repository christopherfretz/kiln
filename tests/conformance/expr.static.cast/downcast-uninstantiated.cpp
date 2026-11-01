// [expr.static.cast]/2, /11: a static_cast between pointers or references
// to a base and a derived class template specialization that has not been
// instantiated yet instantiates it ([temp.inst]/2, the completeness the
// base/derived relation needs; libc++ `static_cast<__node_pointer>(
// __base_pointer)`). Matches clang 19.
struct NodeBase {
  int k = 1;
};
template <class T> struct Node : NodeBase {
  T value;
};
constexpr int down() {
  Node<int> n;
  NodeBase* b = &n;
  return static_cast<Node<int>*>(b)->k + static_cast<Node<int>&>(*b).k;
}
static_assert(down() == 2);
template <class T> constexpr NodeBase& up(Node<T>& n) { return static_cast<NodeBase&>(n); }
template <class T> struct Tree {
  NodeBase* root = nullptr;
  constexpr Node<T>* node() { return static_cast<Node<T>*>(root); }
};
constexpr int use() {
  Node<long> n;
  Tree<short> t;
  return up(n).k + (t.node() == nullptr);
}
static_assert(use() == 2);
