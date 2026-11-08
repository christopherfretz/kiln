// Debug info (WP-814): a constructor with member initializers and a user destructor.
//@ args: -g
struct S {
  int a;
  double b;
  S(int x) : a(x), b(2.0) {}
  ~S() {}
  int get() const { return a + (int)b; }
};
int use(int v) {
  S s(v);
  return s.get();
}
