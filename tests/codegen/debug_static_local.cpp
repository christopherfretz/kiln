// Debug info (WP-814): a function-local static variable.
//@ args: -g
int counter() {
  static int c = 0;
  return ++c;
}
