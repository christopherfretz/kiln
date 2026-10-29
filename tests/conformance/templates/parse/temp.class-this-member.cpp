//@ expect: ast
//@ oracle: clang
// [temp.dep.expr]/2: `this` and members in a class template
template <class T> struct A {
  T x;
  int n;
  T get() { return this->x; }
  int count() { return n + this->n; }
};
