//@ expect: ast
//@ oracle: clang
// [temp.names]/5: the `template` keyword before a dependent member template name
template <class T> int dm(T *p) { return p->template get<int>() + p->x; }
template <class T> int dot(T t) { return t.template get<0>(); }
template <class T> int scoped() { return T::template v<int>; }
