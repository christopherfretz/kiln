// [basic.lookup.general]/1, [expr.prim.id.unqual]/3: where the grammar
// allows an expression, an undeclared identifier is "use of undeclared
// identifier", not "unknown type name": `sizeof(Zork *)`,
// `alignof(Zork)` (an expression operand), a template argument
// (`V<Zork>`), `decltype(Zork)`, `Zork::f()` and `noexcept(Zork)`; a
// declaration statement's decl-specifier and a parameter's are
// "unknown type name" (WP-186).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:26 error "use of undeclared identifier 'Zork'"
//@ diag: 24:32 error "expected expression"
//@ diag: 25:15 error "use of undeclared identifier 'Zork'"
//@ diag: 26:15 error "use of undeclared identifier 'Zork'"
//@ diag: 26:21 error "expected expression"
//@ diag: 27:27 error "use of undeclared identifier 'Zork'"
//@ diag: 27:19 warning "'alignof' applied to an expression is a GNU extension"
//@ diag: 28:22 error "use of undeclared identifier 'Zork'"
//@ diag: 29:13 error "use of undeclared identifier 'Zork'"
//@ diag: 30:20 error "use of undeclared identifier 'Zork'"
//@ diag: 31:13 error "unknown type name 'Zork'"
//@ diag: 32:13 error "unknown type name 'Zork'"
//@ diag: 33:9 error "unknown type name 'Zork'"
template <class T> struct V {};
void a6() { (void)sizeof(Zork *); }
void a7() { V<Zork> v; }
void a8() { V<Zork *> v; }
void b1() { (void)alignof(Zork); }
void b6() { decltype(Zork) q; }
void b9() { Zork::f(); }
void c2() noexcept(Zork);
void c3() { Zork z; }
void c4() { Zork *p; }
void c5(Zork z);
