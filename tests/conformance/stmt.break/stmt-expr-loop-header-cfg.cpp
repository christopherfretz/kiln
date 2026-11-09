// [stmt.break]/1, [stmt.cont]/1, [stmt.return]/4 (GNU statement expressions):
// the CFG behind -Wreturn-type, -Wuninitialized and -Wunreachable-code
// follows clang's analysis CFG, not its codegen, for a jump in a loop
// header: break and continue in a while or do condition, and continue in a
// for condition or increment, go to the enclosing loop; break in a for
// header goes to that loop's exit; break in a switch condition goes to the
// switch's exit. A jump with no target there makes the function's CFG bad,
// which silences these warnings for that function (f1, f3, f6, g7); sema
// still rejects the jumps that have no target at all (g3, g5).
// WP-1075; spec/sema.md §7.4a.
//@ expect: compile-fail
//@ args: -Wall -Wextra -Wunreachable-code-aggressive -Wuninitialized
//@ strict-diags
//@ diag: 22:49 warning "non-void function does not return a value"
//@ diag: 24:71 warning "non-void function does not return a value"
//@ diag: 28:55 warning "non-void function does not return a value"
//@ diag: 29:51 error "'break' statement not in loop or switch statement"
//@ diag: 30:41 warning "unused variable 'x'"
//@ diag: 31:35 error "'break' statement not in loop or switch statement"

int f1() { while (({ break; true; })) {} }
int f2(int n) { for (;; ({ if (n) break; })) {} }
int f3(int n) { do {} while (({ if (n) break; true; })); }
int f4(int n) { while (true) { while (({ if (n) break; true; })) {} } }
int f5(int n) { for (;;) { switch (({ if (n) break; n; })) { default: return 1; } } }
int f6(int n) { int x; while (({ if (n) break; true; })) { x = 1; } return x; }
int g1(int n) { while (true) { for (; ({ if (n) continue; true; });) {} } }
int g2(int n) { for (; ({ if (n) break; true; });) {} }
int g3(int n) { int a[1]; for (int x : *({ if (n) break; &a; })) {} }
int g4(int n) { while (true) { for (int x : *({ int a[1]; if (n) continue; &a; })) {} } }
int g5(int n) { switch (({ if (n) break; n; })) { default: return 1; } }
int g6(int n) { while (true) { switch (({ if (n) continue; n; })) { default: return 1; } } }
int g7(int n) { while (({ if (n) continue; true; })) {} }
