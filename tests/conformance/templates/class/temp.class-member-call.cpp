//@ expect: ast
//@ oracle: clang
// [temp.class]/1, [temp.inst]/5: calling a member function of a
// specialization instantiates its definition.
template <class T> struct Counter {
  T n;
  T next() { return ++n; }
  T peek() const { return n; }
};
int g(Counter<int> *c) { return c->next(); }
