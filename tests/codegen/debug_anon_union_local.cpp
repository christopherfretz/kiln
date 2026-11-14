// Debug info (WP-1400): a block-scope anonymous union ([class.union.anon]/1)
// is one artificial unnamed variable, preceded (clang `EmitDeclare`) by
// an artificial variable per member on the same storage, with no file
// and line 0, so a debugger finds `i` and `fl` by name. An unnamed member
// is skipped unless it is a class (the nested anonymous struct); a
// parameter or a named variable of a class with an anonymous union member
// gets no extra variables.
//@ args: -g
struct H {
  union {
    long b;
    char c;
  };
};
int use(int k, H h) {
  union {
    int i;
    float fl;
    struct {
      short lo, hi;
    };
    int : 3;
  };
  i = k;
  H named = h;
  return i + (int)fl + lo + (int)named.b;
}
