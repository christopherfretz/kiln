struct S {
  char c;
  int i;
} PACKED;
inline int f(int a) {
  S s{1, a};
  return s.i + (int)sizeof(S);
}
