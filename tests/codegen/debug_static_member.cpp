// Debug info (WP-814): a static data member's definition and use.
//@ args: -g
struct S {
  static int count;
  int v;
};
int S::count = 3;
int use(S s) { return s.v + S::count; }
