// Debug info (WP-814): a for loop's scope, condition and increment stop points.
//@ args: -g
int sum(int n) {
  int r = 0;
  for (int i = 0; i < n; ++i)
    r += i;
  return r;
}
