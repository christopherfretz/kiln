// implicit
struct M {
  M();
  M(const M &);
};
struct S {
  M m;
  int v = 0;
};
inline S copy(const S &s) { return S(s); }
int use_b() { S s; return copy(s).v; }
