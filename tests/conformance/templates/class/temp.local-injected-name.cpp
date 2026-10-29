//@ expect: ast
//@ oracle: clang
// [temp.local]/1-2: inside a class template the injected-class-name names
// the current specialization, both as a type and as a template-name.
template <class T> struct Node {
  Node *next;
  Node<T> *prev;
  static int count(const Node &n) { return n.next ? 1 : 0; }
};
Node<int> *head;
int k = sizeof(Node<int>);
