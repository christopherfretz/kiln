// Debug info (WP-814): a global variable, a reference local and a reference parameter.
//@ args: -g
int g = 3;
const int *gp = &g;
int touch(int &r) {
  int &ref = g;
  r += ref;
  return *gp;
}
