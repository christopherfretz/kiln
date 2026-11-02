//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 18:48 error "declaration of 'Ts' shadows template parameter"
//@ diag: 18:20 note "template parameter is declared here"
//@ diag: 18:50 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 19:58 error "'const' is invalid in friend declarations"
//@ diag: 19:67 error "'volatile' is invalid in friend declarations"
//@ diag: 20:24 error "cannot combine with previous 'type-name' declaration specifier"
//@ diag: 20:27 error "expected ';' after friend declaration statement"
//@ diag: 20:28 error "a type specifier is required for all declarations"
//@ diag: 21:26 error "expected ';' after friend declaration statement"
//@ diag: 21:27 error "expected member name or ';' after declaration specifiers"
// [class.friend]/3 (P2893), [temp.local]/6: `friend class Ts...` declares
// a new class Ts (shadowing the pack), so its `...` expands nothing; cv-
// qualifiers are rejected per element; a list missing its `;` skips nothing
// and the rest is parsed as the next member declaration.
template <class... Ts> class C9 { friend class Ts...; };
struct A; struct B; template <class T> class C1 { friend const T, volatile A; };
class C2 { friend A, B int y; };
class C3 { friend A, void(); };
