// [class.bit]/1: a bit-field width is a non-negative constant, at most the type width for bool
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:7 error "bit-field 'neg' has negative width (-1)"
//@ diag: 9:7 warning "width of bit-field 'x' (40 bits) exceeds the width of its type; value will be truncated to 32 bits"
struct B {
  int neg : -1;
  bool wide : 2;
  int x : 40;
};
