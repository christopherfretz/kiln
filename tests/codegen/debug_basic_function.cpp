// Debug info (WP-814): a function's subprogram, parameters, locals, line table.
//@ args: -g
static int helper(int a, int b) {
  int r = a;
  r += b;
  return r;
}
int entry(int v) { return helper(v, 2); }
