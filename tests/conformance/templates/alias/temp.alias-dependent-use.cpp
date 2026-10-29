//@ expect: ast
//@ oracle: clang
// [temp.alias]/2, [temp.dep.type]: an alias template used with a dependent
// argument inside a class template.
template <class T> using Ptr = T *;
template <class T> struct Node { Ptr<Node> next; Ptr<T> value; };
int sz = sizeof(Node<int>);
