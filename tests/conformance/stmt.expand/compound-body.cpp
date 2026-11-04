// [stmt.expand]/1 (P1306R5): the body of an expansion statement is a
// compound-statement. Any other statement is accepted as an extension
// (clang 23, -Wexpansion-stmt-missing-braces, off by default; GCC 16.1
// accepts it even with -pedantic-errors), as are attributes before the
// compound statement. Labels and attributes are looked through: a
// compound statement after `[[]]` gets the attribute warning only.
// Run-time behaviour: non-compound-body-run.cpp.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
//@ args: -Wexpansion-stmt-missing-braces
//@ diag: 20:34 warning "ISO C++ requires the body of an expansion statement to be a compound statement"
//@ diag: 21:34 warning "ISO C++ forbids attributes before the compound statement of an expansion statement"
//@ diag: 22:34 warning "ISO C++ requires the body of an expansion statement to be a compound statement"
//@ diag: 23:34 warning "ISO C++ requires the body of an expansion statement to be a compound statement"
//@ diag: 24:34 warning "ISO C++ requires the body of an expansion statement to be a compound statement"
//@ diag: 26:64 warning "ISO C++ requires the body of an expansion statement to be a compound statement"
int f() {
  int s = 0;
  template for (auto x : {1, 2}) s += x;
  template for (auto x : {1, 2}) [[]] { s += x; }
  template for (auto x : {1, 2}) [[]] s += x;
  template for (auto x : {1, 2}) if (x) s += x;
  template for (auto x : {1, 2}) ;
  template for (auto x : {1, 2}) { s += x; }
  template for (auto x : {1, 2}) { template for (auto y : {x}) s += y; }
  return s;
}
