// [stmt.break]/1, [stmt.cont]/1, [stmt.for]/1 (GNU statement expressions):
// a break or continue inside a statement expression needs an enclosing
// loop (or, for break, switch) that is open at that point. A for loop's
// init-statement, a range-based for's init and range, and a switch's
// condition belong to the enclosing statement, so a jump there with no
// outer loop is an error, as is one inside a lambda (a new function). A
// continue in a for loop's condition-variable initializer would skip the
// variable's initialization on its way to the increment, which clang
// rejects (err_continue_from_cond_var_init); a while loop's condition
// variable is re-declared at the condition, so that continue is valid.
// WP-1075; spec/sema.md §7.4a.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:21 error "'break' statement not in loop or switch statement"
//@ diag: 27:28 error "'continue' statement not in loop statement"
//@ diag: 28:29 error "'break' statement not in loop or switch statement"
//@ diag: 29:48 error "cannot jump from this continue statement to the loop increment; jump bypasses initialization of loop condition variable"
//@ diag: 30:31 error "cannot jump from this continue statement to the loop increment; jump bypasses initialization of loop condition variable"
//@ diag: 31:21 error "'continue' statement not in loop statement"
//@ diag: 31:31 warning "expression result unused"
//@ diag: 32:39 error "'break' statement not in loop or switch statement"
//@ diag: 33:24 error "'break' statement not in loop or switch statement"
//@ diag: 34:70 error "'continue' statement not in loop statement"
//@ diag: 37:54 warning "expression result unused"

int f() { return ({ break; 1; }); }
void g(int n) { switch (({ continue; n; })) {} }
void g2(int n) { switch (({ break; n; })) {} }
void h(int a[3]) { for (; int x = ({ if (a[0]) continue; 1; }); ++x) {} }
void h2() { for (; int x = ({ continue; 1; });) {} }
void k2() { for (({ continue; 0; }); ;) {} }
void r() { int a[2]; for (int x : *({ break; &a; })) {} }
void l() { while ([] { break; return true; }()) {} }
template <class T> void t() { for (; ({ break; T(); });) {} T y = ({ continue; T(); }); }

// Valid twins: each jump finds an open loop.
void k(int n) { while (n) { for (({ if (n) continue; 0; }); ;) {} } }
void d(int n) { do {} while (({ if (n) continue; 0; })); }
void s(int n) { while (true) { switch (n) { case 1: switch (({ if (n) continue; n; })) {} } } }
void w(int n) { while (int x = ({ if (n) continue; 1; })) {} }
void wl(int n) { while (n) { auto f = [] { while (({ break; true; })) {} return 0; }; (void)f; } }
