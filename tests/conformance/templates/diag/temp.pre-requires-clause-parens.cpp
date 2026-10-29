//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:34 error "parentheses are required around this expression in a requires clause"
//@ diag: 8:39 error "parentheses are required around this expression in a requires clause"
// [temp.pre]/9: a requires-clause operand is a primary expression
template <class T> constexpr bool C = true;
template <class T> requires C<T> == true void f();
template <class T> requires sizeof(T) == 4 void g();
