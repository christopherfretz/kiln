// Bit-field default member initializers and mem-initializers in a
// constructor ([class.base.init]/9): each is stored through its storage
// unit (load, mask, or, store), not as a whole-byte store at the unit's
// start (WP-286; libc++ `__format_spec::__parser`).
struct Q {
  unsigned char a : 8 {0};
  unsigned char b : 6 {0};
  bool c : 1 {false};
  int d : 5;
  Q() : d(-3) {}
  Q(int x) : a(1), b(x), c(true), d(x) {}
};
Q make() { return Q(); }
Q make2(int x) { return Q(x); }
