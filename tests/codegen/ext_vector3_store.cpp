// [basic.fundamental] (clang ext_vector_type extension): a 3-lane ext
// vector is stored through its 4-lane memory type. The value is widened
// with a shufflevector: at compat 22 lane 3 comes from an `undef`
// operand with mask <0, 1, 2, 3>; at compat 19 the second operand is
// `poison` and lane 3 of the mask is poison (WP-466).
typedef int i3 __attribute__((ext_vector_type(3)));
typedef float f3 __attribute__((ext_vector_type(3)));
void st(i3 *p, i3 v) { *p = v; }
void stf(f3 *p, f3 v) { *p = v; }
void stv(volatile i3 *p, i3 v) { *p = v; }
i3 g;
void sg(i3 v) { g = v; }
i3 loc(i3 v) {
  i3 t = v + 1;
  return t;
}
