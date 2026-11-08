// Debug info (WP-814): a named return value (the variable lives in the return slot).
//@ args: -g
struct S {
  int a;
  long b[4];
  ~S() {}
};
S make(int v) {
  S s;
  s.a = v;
  return s;
}
int use() { return make(4).a; }
