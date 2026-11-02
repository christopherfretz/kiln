//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:12 error "a type specifier is required for all declarations"
//@ diag: 11:19 error "a type specifier is required for all declarations"
//@ diag: 12:13 error "expected member name or ';' after declaration specifiers"
//@ diag: 13:12 error "expected member name or ';' after declaration specifiers"
// [dcl.spec.general]/3, [class.mem.general]: a member declaration needs a
// type; a name that is not a type, followed by `;` or `=`, is a declarator
// without one (clang's implicit-int recovery).
class Z1 { y; };
class Z3 { int x; y = 2; };
class Z2 { (); };
class Z4 { 1; };
