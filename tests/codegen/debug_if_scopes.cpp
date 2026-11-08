// Debug info (WP-814): an if statement's lexical block and its locals.
//@ args: -g
int pick(int a) {
  int r = a;
  if (a > 1) {
    int t = a * 2;
    r += t;
  } else {
    r -= 1;
  }
  return r;
}
