// -Wunused-value on statement expressions looks at their last statement
// (through one label); the result statement itself is not discarded. A
// statement expression whose last statement is not an expression is
// void, and 'auto' cannot deduce an object type from it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:7 error "cannot initialize a variable of type 'int' with an rvalue of type 'void'"
//@ diag: 33:8 error "variable has incomplete type 'void'"
//@ diag: 34:8 error "variable has incomplete type 'void'"
//@ diag: 23:6 warning "expression result unused"
//@ diag: 23:9 warning "expression result unused"
//@ diag: 24:6 warning "expression result unused"
//@ diag: 24:3 warning "expression result unused"
//@ diag: 25:9 warning "expression result unused"
//@ diag: 27:17 warning "expression result unused"
//@ diag: 28:9 warning "expression result unused"
//@ diag: 30:3 warning "expression result unused"
//@ diag: 31:27 warning "expression result unused"
//@ diag: 32:24 warning "expression result unused"
//@ diag: 33:16 warning "expression result unused"
int g;
void f() {
  ({ 1; 2; });
  ({ 1; 2; ; });
  ({ l: 3; });
  ({ g = 1; });
  ({ int q = 1; q; });
  ({ ({ 4; }); });
  int k = ({ ; });
  sizeof(({ 3; }));
  switch (1) { case 1: ({ 2; }); }
  ({ int z = ({ g; }); z; });
  auto a = ({ {7;} });
  auto b = ({ (void)g; });
}
