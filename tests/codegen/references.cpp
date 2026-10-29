// Reference parameters and returns, including references to structs.
struct V { int x, y; };
int& pick(int& a, int& b, bool first) { return first ? a : b; }
void bump(V& v) { v.x++; v.y += 2; }
const V& cref(const V& v) { return v; }
int refs(int a, int b) {
  pick(a, b, true) = 5;
  V v = {a, b};
  bump(v);
  return cref(v).x + v.y + a;
}
