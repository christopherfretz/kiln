// Debug info (WP-814): a lambda's closure type and its captures.
//@ args: -g
int run(int q) {
  int b = 2;
  auto l = [q, &b](int y) { return q + y + b; };
  return l(3);
}
