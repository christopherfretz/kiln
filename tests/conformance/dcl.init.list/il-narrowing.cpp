// [dcl.init.list]/5: the backing array's elements are copy-initialized
// from the list, so narrowing conversions are ill-formed.
//@ expect: compile-fail
//@ diag: 12:36 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 12:36 note "insert an explicit cast to silence this issue"
//@ diag: 13:34 error "constant expression evaluates to 300 which cannot be narrowed to type 'char'"
//@ diag: 13:34 note "insert an explicit cast to silence this issue"
//@ diag: 15:19 error "constant expression evaluates to -1 which cannot be narrowed to type 'unsigned int'"
//@ diag: 15:19 note "insert an explicit cast to silence this issue"
#include "Inputs/initializer_list.h"
double d = 1.5;
std::initializer_list<int> a = {1, d};
std::initializer_list<char> b = {300};
void f(std::initializer_list<unsigned>);
void g() { f({1u, -1}); }
