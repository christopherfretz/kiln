// [dcl.typedef], [temp.inst]: a block-scope typedef or alias-declaration
// whose type fails to substitute is declared invalid with type `int`, as
// clang's InstantiateTypedefNameDecl recovers it: its uses are checked with
// that type and diagnosed, and the rest of the body is instantiated
// (WP-848; kiln dropped the declaration and every later use was silent).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 21:23 error "no type named 'nope' in 'X'"
//@ diag: 36:9 note "in instantiation of function template specialization 'typedef_use<X>' requested here"
//@ diag: 22:7 error "no viable conversion from 'X' to 'bad' (aka 'int')"
//@ diag: 27:27 error "no type named 'value_type' in 'X'"
//@ diag: 37:9 note "in instantiation of function template specialization 'alias_use<X>' requested here"
//@ diag: 28:7 error "cannot initialize a variable of type 'bad' (aka 'int') with an lvalue of type 'const char[2]'"
//@ diag: 32:23 error "no type named 'nope' in 'X'"
//@ diag: 38:9 note "in instantiation of function template specialization 'later_statement<X>' requested here"
//@ diag: 34:12 error "invalid operands to binary expression ('X' and 'bad' (aka 'int'))"
struct X {};

template <class T> int typedef_use(T t) {
  typedef typename T::nope bad;
  bad b = t;
  bad c = 0;
  return b + c;
}
template <class T> int alias_use(T) {
  using bad = typename T::value_type;
  bad b = "x";
  return b;
}
template <class T> int later_statement(T t) {
  typedef typename T::nope bad;
  bad b = 1;
  return t + b;
}
int a = typedef_use(X());
int b = alias_use(X());
int c = later_statement(X());
