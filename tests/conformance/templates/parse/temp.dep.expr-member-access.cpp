//@ expect: ast
//@ oracle: clang
// [temp.dep.expr]/5: class member access on a type-dependent object
template <class T> int f(T t) { return t.x + t.g(1); }
template <class T> int h(T *p) { return p->x; }
template <class T> int k(T &r) { return r.x; }
