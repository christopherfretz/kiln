// The module of module_used_inline_body_importer.cpp: inline functions
// with GNU `used` that the importer reaches only through the bodies of
// other imported inline and constexpr functions.
export module mod_used_inline_body;
inline __attribute__((used)) int b1() { return 1; }
inline __attribute__((used)) int b2() { return 2; }
inline __attribute__((used)) int b3() { return 3; }
inline __attribute__((used)) int b4() { return 4; }
inline __attribute__((used)) int b5() { return 5; }
inline __attribute__((used)) int b6() { return 6; }
inline __attribute__((used)) int b7() { return 7; }
inline __attribute__((used)) int b8() { return 8; }
inline __attribute__((used)) int b9() { return 9; }
inline __attribute__((used)) int b10() { return 10; }
inline __attribute__((used)) int b11() { return 11; }
inline __attribute__((used)) int b12() { return 12; }
inline __attribute__((used)) int b13() { return 13; }
inline __attribute__((used)) int b14() { return 14; }
inline __attribute__((used)) int b15() { return 15; }
inline __attribute__((used)) constexpr int c1() { return 21; }
inline __attribute__((used)) constexpr int c2() { return 22; }
inline int plain() { return 30; }
inline int dflt(int p = b10()) { return p; }
inline int inner() { return b15(); }
export inline int order() { return b3() + b1() + b2() + inner(); }
export inline int branch() {
  int x = b4();
  if (b5())
    return x;
  return b6();
}
export inline int loops(int v) {
  int r = plain();
  do {
    r += b7();
  } while (b8() == 0);
  switch (b9(); v) {
  case 1:
    return r;
  }
  return r;
}
export inline int defarg() { return dflt(); }
export inline int lam() { return [] { return b11(); }(); }
export inline int unevaluated() { return b12(); }
export inline int twice() { return b13() + b13(); }
export inline int tmpl() { return b14(); }
export constexpr int ce() { return c1(); }
export constexpr int ce2() { return c2(); }
