// [expr.log.and], [stmt.if], [cpp.replace] (WP-419): a fix-it whose end
// lies inside a macro expansion (the constant of 'a && G(5)', the
// '!' of 'X 1') is dropped, but -Wconstant-logical-operand,
// -Wunreachable-code and their notes are still emitted; ends at the
// edge of an expansion or in a macro argument keep the note
// unchanged. The fix-its themselves are checked against clang's
// stderr in kiln_driver's fixits_in_macro_expansions_like_clang
// (spec/sema.md §7.10.1).
//@ args: -Wunreachable-code
//@ strict-diags
//@ diag: 37:29 warning "use of logical '&&' with constant operand"
//@ diag: 37:29 note "use '&' for a bitwise operation"
//@ diag: 37:29 note "remove constant to silence this warning"
//@ diag: 38:31 warning "use of logical '&&' with constant operand"
//@ diag: 38:31 note "use '&' for a bitwise operation"
//@ diag: 38:31 note "remove constant to silence this warning"
//@ diag: 39:27 warning "use of logical '&&' with constant operand"
//@ diag: 39:27 note "use '&' for a bitwise operation"
//@ diag: 39:27 note "remove constant to silence this warning"
//@ diag: 40:27 warning "use of logical '&&' with constant operand"
//@ diag: 40:27 note "use '&' for a bitwise operation"
//@ diag: 40:27 note "remove constant to silence this warning"
//@ diag: 41:24 warning "code will never be executed"
//@ diag: 41:17 note "silence by adding parentheses to mark code as explicitly dead"
//@ diag: 35:13 note "expanded from macro 'NOT'"
//@ diag: 42:22 warning "code will never be executed"
//@ diag: 42:17 note "silence by adding parentheses to mark code as explicitly dead"
//@ diag: 36:18 note "expanded from macro 'X'"
void g();
bool h();
#define LHS a
#define ID(x) x
#define G(x) x; (void)0
#define F5 5; (void)0
#define NOT !
#define X h() && !
bool f1(int a) { return LHS && 5; }
bool f2(int a) { return ID(a) && ID(5); }
bool f3(int a) { return a && G(5); }
bool f4(int a) { return a && F5; }
void u1() { if (NOT 1) g(); }
void u2() { if (X 1) g(); }
