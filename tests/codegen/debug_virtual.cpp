// Debug info (WP-814): a polymorphic class hierarchy: vtable pointer, virtual methods, override.
//@ args: -g
struct Base {
  virtual ~Base() {}
  virtual int f() { return 1; }
  int b = 0;
};
struct Der : Base {
  int f() override { return b + 2; }
};
int call(Der &d) { return d.f(); }
