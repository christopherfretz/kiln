// [class.bit]/1: a substituted bit-field width must be non-negative and a
// named bit-field must have a non-zero width.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:34 error "bit-field 'b' has negative width (-1)"
//@ diag: 10:8 note "in instantiation of template class 'BF<-1>' requested here"
//@ diag: 9:34 error "named bit-field 'b' has zero width"
//@ diag: 11:7 note "in instantiation of template class 'BF<0>' requested here"
template <int N> struct BF { int b : N; };
BF<-1> x;
BF<0> y;
BF<3> ok;
