// The module of used-inline-body-run.cpp: inline functions with GNU
// `used` reached only through other inline functions' bodies.
export module usedbody;
inline __attribute__((used)) int u1() { return 1; }
inline __attribute__((used)) int u2() { return 2; }
inline __attribute__((used)) int u3() { return 3; }
inline __attribute__((used)) int u4() { return 4; }
inline __attribute__((used)) constexpr int u5() { return 5; }
inline int inner() { return u4(); }
export inline int sum() { return u2() + u1() + inner(); }
export inline int pick(int v) {
  if (u3() == v)
    return v;
  return 0;
}
export constexpr int ce() { return u5(); }
export using fp = int (*)();
export inline fp addr() { return &u1; }
export fp mod_addr();
fp mod_addr() { return addr(); }
