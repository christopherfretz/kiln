//@ expect: ast
// [stmt.if]/4: `if consteval` and `if !consteval`; the consteval branch
// is an immediate function context, so its calls are not folded.
consteval int sq(int x) { return x * x; }
constexpr int pick(int x) {
  if consteval {
    return sq(x);
  } else {
    return x;
  }
}
constexpr int neg(int x) {
  if !consteval {
    return -x;
  }
  return sq(4) + x;
}
int run(int n) { return pick(n) + neg(n); }
