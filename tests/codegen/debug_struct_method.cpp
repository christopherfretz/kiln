// Debug info (WP-814): a struct with members and a const inline method.
//@ args: -g
struct P {
  int x;
  int y;
  int sum() const { return x + y; }
};
int use(int a) {
  P p{a, 2};
  return p.sum();
}
