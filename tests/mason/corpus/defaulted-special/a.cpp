// = default (constexpr is implicit from C++20 only)
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
int use_a() { S s; return copy(s).v; }
