// Debug info (WP-814): an if statement with an init-statement and a condition variable scope.
//@ args: -g
int sw(int v) {
  if (int q = v * 2; q > 3)
    return q;
  else
    return -q;
}
