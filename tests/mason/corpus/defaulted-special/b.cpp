// = default
struct M {
  M();
  M(const M &);
};
struct S {
  M m;
  int v = 0;
  S() = default;
  S(const S &) = default;
};
inline S copy(const S &s) { return S(s); }
int use_b() { S s; return copy(s).v; }
