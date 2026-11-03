// [stmt.expand]: before C++26 an expansion statement is accepted as an
// extension with clang's -Wc++26-extensions warning; `template` followed by
// anything but `for` in a function body keeps its existing error.
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ requires: clang>=23
//@ diag: 12:3 warning "expansion statements are a C++26 extension"
//@ diag: 13:3 error "'template' here is not supported yet"
int f() {
  int s = 0;
  template for (auto x : {1, 2}) { s += x; }
  template int y;
  return s;
}
