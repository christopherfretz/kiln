// CWG 2631, [expr.const]/16, [dcl.fct.default]/5: an immediate invocation
// in a default argument, or in a default argument that one uses, is not
// evaluated where it is written but at each use of the outermost one, in
// that use's context: `pick(__builtin_FUNCTION())` fails at a use in `fn`
// or `ab` (too short a name), not at the declarations, and not in
// `long_name`; a nested one is reported at its own use with its parameter
// (clang 19 and 22). Reduced from real code.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:17 error "call to consteval function 'pick' is not a constant expression"
//@ diag: 29:18 note "in the default initializer of 'a'"
//@ diag: 27:9 note "declared here"
//@ diag: 26:44 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 27:17 note "in call to 'pick(&\"fn\"[0])'"
//@ diag: 27:17 error "call to consteval function 'pick' is not a constant expression"
//@ diag: 29:18 note "in the default initializer of 'a'"
//@ diag: 27:9 note "declared here"
//@ diag: 26:44 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 27:17 note "in call to 'pick(&\"ab\"[0])'"
//@ diag: 27:17 error "call to consteval function 'pick' is not a constant expression"
//@ diag: 35:19 note "in the default initializer of 'a'"
//@ diag: 27:9 note "declared here"
//@ diag: 26:44 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 27:17 note "in call to 'pick(&\"ab\"[0])'"
consteval int bad(int x) { if (x) throw 1; return 0; }
consteval int pick(const char *s) { return s[3]; }
void k2(int a = pick(__builtin_FUNCTION()));
void k(int a = bad(1));
void h2(int b = (k2(), 0));
void h(int b = (k(), 0));
struct S2 { int m = pick(__builtin_FUNCTION()); };
struct S3 { int m = (k2(), 0); };
void fn() { h2(); }
void long_name() { h2(); k2(); S2 s{}; S3 t{}; }
void ab() { h2(); k2(); }
