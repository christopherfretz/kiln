// [stmt.expand]: before C++26 an expansion statement is accepted as an
// extension with clang's -Wc++26-extensions warning. (`template` followed by
// anything but `for` in a block: temp.explicit/block-scope-explicit-*.)
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ requires: clang>=23
//@ diag: 11:3 warning "expansion statements are a C++26 extension"
int f() {
  int s = 0;
  template for (auto x : {1, 2}) { s += x; }
  return s;
}
