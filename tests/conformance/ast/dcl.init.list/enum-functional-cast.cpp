//@ expect: ast
// [dcl.init.list]/3.8 through [expr.type.conv]/2: `E{v}` for an
// enumeration with a fixed underlying type converts `v` to the
// underlying type, then to `E` (WP-170: std::align_val_t{32}).
enum class E : unsigned long {};
enum F : int {};
E take(E e) { return e; }
E g(int n) { return take(E{3}); }
F h(short s) { return F{s}; }
