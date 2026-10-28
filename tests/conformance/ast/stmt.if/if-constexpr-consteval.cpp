//@ expect: ast
constexpr int pick(int v) {
  if constexpr (sizeof(int) == 4) {
    v += 1;
  } else {
    v -= 1;
  }
  if consteval {
    return v * 2;
  } else {
    return v;
  }
}
constexpr int k = pick(3);
int rt(int v) {
  if !consteval {
    return pick(v);
  }
}
