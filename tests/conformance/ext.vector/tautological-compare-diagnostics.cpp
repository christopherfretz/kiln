// GNU and ext vector comparisons, [expr.rel]/[expr.eq] applied lane-wise:
// comparing an integer vector with itself is -Wtautological-compare
// ("self-comparison always evaluates to a constant"), as for a scalar; a
// floating-point vector is not (a NaN lane compares unequal). WP-187.
//@ strict-diags
//@ diag: 13:26 warning "self-comparison always evaluates to true"
//@ diag: 14:26 warning "self-comparison always evaluates to false"
//@ diag: 15:26 warning "self-comparison always evaluates to false"
//@ diag: 16:26 warning "self-comparison always evaluates to true"
typedef int g4i __attribute__((vector_size(16)));
typedef int v4i __attribute__((ext_vector_type(4)));
typedef float v4f __attribute__((ext_vector_type(4)));
g4i t1(g4i a) { return a == a; }
v4i t2(v4i a) { return a < a; }
g4i t3(g4i a) { return a != a; }
v4i t4(v4i a) { return a >= a; }
v4i t5(v4f a) { return a == a; }
v4i t6(v4i a, v4i b) { return a == b; }
