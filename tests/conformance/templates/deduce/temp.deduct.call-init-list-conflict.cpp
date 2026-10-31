// [temp.deduct.call]/1: conflicting deductions from the elements of a
// braced-init-list fail deduction.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:12 error "no matching function for call to 'f'"
//@ diag: 8:25 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'double')"
#include "../../dcl.init.list/Inputs/initializer_list.h"
template <class T> void f(std::initializer_list<T>);
void g() { f({1, 2.0}); }
