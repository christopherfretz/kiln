// [stmt.switch]/2: a switch on a `_BitInt(N)` with N > 128 (and on an
// `unsigned __int128`) lowers to an LLVM `switch` on the condition's `iN`
// with each case's exact value, also one outside `i128`; a non-type template
// argument of a wide type is a literal of its value. Where BITINT_MAXWIDTH
// allows (x86_64); the `unsigned __int128` cases on both targets.
using u128 = unsigned __int128;
int su(u128 v) {
  switch (v) {
  case ~u128(0): return 1;
  case u128(1) << 127: return 2;
  case 3: return 3;
  }
  return 0;
}
#if __BITINT_MAXWIDTH__ >= 256
using W = _BitInt(200);
using U = unsigned _BitInt(256);
int sw(W v) {
  switch (v) {
  case 1: return 10;
  case -1: return 20;
  case W(1) << 160: return 40;
  case -(W(1) << 190): return 50;
  default: return 30;
  }
}
int sw2(U v) {
  switch (v) {
  case 0: return 1;
  case ~U(0): return 2;
  }
  return 3;
}
template <W V> W lit() { return V; }
W use() { return lit<(W(1) << 160)>() + lit<-1>(); }
#endif
