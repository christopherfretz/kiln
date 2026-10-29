//@ expect: ast
//@ oracle: clang
// [temp.dep.expr]/1: operators with type-dependent operands
template <class T> void f(T t, T u) {
  t = u;
  t += u;
  -t;
  !t;
  t[0];
  t < u;
  t ? t : u;
  ++t;
  t++;
}
