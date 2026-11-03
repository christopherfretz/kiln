// [temp.names]/3 (P0846): an unqualified operator-function-id that lookup
// finds nothing for, followed by `<`, is assumed to be a template-name;
// with no call to find a template by argument-dependent lookup it is an
// undeclared name. Qualified, a non-static member named without an object
// is diagnosed as such and `<` is the less-than operator (other
// non-templates: operator-function-id-non-template-args-22.cpp).
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ diag: 16:12 error "call to non-static member function without an object argument"
//@ diag: 16:26 error "expected '(' for function-style cast or type construction"
//@ diag: 17:29 error "use of undeclared 'operator+'"
//@ diag: 18:9 error "use of undeclared 'operator-'"
struct S {
  int operator()(int) const;
};
int a = S::operator()<int>(1);
struct T { int h() { return operator+<int>(1); } };
int b = operator-<int>;
