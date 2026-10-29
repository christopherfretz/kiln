//@ expect: ast
//@ oracle: clang
// [temp.local]/1: the injected-class-name of a class template
template <class T> struct Node {
  Node *next;
  Node<T> *prev;
  T value;
  Node *self() { return this; }
};
