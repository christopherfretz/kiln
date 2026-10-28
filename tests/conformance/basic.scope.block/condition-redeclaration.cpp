// [basic.scope.block]/2: the outermost block of a sub-statement cannot redeclare the condition's names
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:22 error "redefinition of 'x'"
//@ diag: 17:11 note "previous definition is here"
//@ diag: 18:33 error "redefinition of 'y'"
//@ diag: 18:11 note "previous definition is here"
//@ diag: 19:37 error "redefinition of 'i'"
//@ diag: 19:12 note "previous definition is here"
//@ diag: 20:42 error "redefinition of 'w'"
//@ diag: 20:14 note "previous definition is here"
//@ diag: 21:36 error "redefinition of 's'"
//@ diag: 21:15 note "previous definition is here"
//@ diag: 22:35 error "redefinition of 'k'"
//@ diag: 22:11 note "previous definition is here"
void f(int a) {
  if (int x = a) int x = 2;
  if (int y = a) { } else { int y = 1; }
  for (int i = 0; i < 3; ++i) { int i = 2; }
  while (int w = a) { { int w = 1; } int w = 3; }
  switch (int s = a) { case 1: int s = 3; }
  if (int k = 1; int j = k) { int k = 3; }
}
