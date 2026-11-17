struct S {
  int a;
  int b = VAL;
};
inline int f(int a) {
  S s{a};
  return s.b;
}
