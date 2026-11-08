// Debug info (WP-814): a range-based for loop and its implicit variables.
//@ args: -g
int total(int (&a)[3]) {
  int s = 0;
  for (int &x : a)
    s += x;
  return s;
}
