struct alignas(A) S {
  char c;
};
inline int f(int a) {
  S s{char(a)};
  return s.c + (int)alignof(S);
}
