// [temp.arg.nontype]/1, [expr.const]/12: a converted constant expression
// allows no narrowing conversion.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:3 error "non-type template argument evaluates to 300, which cannot be narrowed to type 'unsigned char'"
//@ diag: 10:3 error "non-type template argument evaluates to 2, which cannot be narrowed to type 'bool'"
template <unsigned char C> struct B {};
B<300> b;
template <bool V> struct F {};
F<2> f;
