// Debug info (WP-814): bit-field members.
//@ args: -g
struct F {
  unsigned a : 3;
  unsigned b : 5;
  int c;
};
int set(F &f) {
  f.a = 5;
  f.b = 1;
  return f.a + f.c;
}
