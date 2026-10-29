//@ expect: ast
//@ oracle: clang
// [temp.inst]/4, [class.mfct.non.static]: a const member function of a
// specialization is instantiated on use; `this` substitutes to the
// specialization type.
template <class T> struct Counter {
  T n;
  T get() const { return this->n; }
  void bump() { ++n; }
};
long use(Counter<long> *c) { c->bump(); return c->get(); }
