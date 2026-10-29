// [dcl.spec.auto.general]/9: each return statement must deduce the same type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:3 error "'auto' in return type deduced as 'double' here but deduced as 'int' in earlier return statement"
//@ diag: 15:3 error "'auto' in return type deduced as 'int' here but deduced as 'void' in earlier return statement"
//@ diag: 21:3 error "'decltype(auto)' in return type deduced as 'int &' here but deduced as 'int' in earlier return statement"
auto f(bool b) {
  if (b)
    return 1;
  return 2.0;
}
auto g(bool b) {
  if (b)
    return;
  return 1;
}
decltype(auto) h(bool b) {
  static int i;
  if (b)
    return i;
  return (i);
}
