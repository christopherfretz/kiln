// Debug info (WP-814): explicit casts keep their types in the retained types.
//@ args: -g
struct A {
  int v;
};
long f(void *p, double d) {
  A *a = static_cast<A *>(p);
  return (long)d + a->v + static_cast<short>(d);
}
