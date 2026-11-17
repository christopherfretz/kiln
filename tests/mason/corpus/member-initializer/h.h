struct S {
  int v = VAL;
};
inline int f(int a) {
  S s;
  return s.v + a;
}
