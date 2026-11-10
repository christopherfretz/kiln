// [module.interface]/1, [dcl.meaning.general]/2: directly in an export
// (also through `export { }`), a class or enumeration defined with a
// qualified name cannot name the global scope (`export struct ::X {}`,
// clang's diagnoseQualifiedDeclaration from ActOnTag); the definition is
// still checked as an exported redeclaration. A namespace member's
// qualified definition is checked against its previous declaration's
// linkage only, with no -Wextra-qualification inside its namespace
// (WP-1174).
//@ expect: compile-fail
//@ std: c++26
//@ args: -Wextra-qualification
//@ strict-diags
//@ diag: 30:17 error "definition or redeclaration of 'X' cannot name the global scope"
//@ diag: 30:17 error "cannot export redeclaration 'X' here since the previous declaration has module linkage"
//@ diag: 25:8 note "previous declaration is here"
//@ diag: 31:19 error "definition or redeclaration of 'Y' cannot name the global scope"
//@ diag: 32:21 error "definition or redeclaration of 'E' cannot name the global scope"
//@ diag: 32:21 error "cannot export redeclaration 'E' here since the previous declaration has module linkage"
//@ diag: 27:12 note "previous declaration is here"
//@ diag: 33:18 error "cannot export redeclaration 'Z' here since the previous declaration has module linkage"
//@ diag: 28:22 note "previous declaration is here"
//@ diag: 34:37 error "cannot export redeclaration 'TZ' here since the previous declaration has module linkage"
//@ diag: 28:51 note "previous declaration is here"
export module m;
struct X;
export struct Y;
enum class E : int;
namespace A { struct Z; template <class T> struct TZ; }
export namespace B { struct W; }
export struct ::X {};
export { struct ::Y {}; }
export enum class ::E : int { e };
export struct A::Z {};
export template <class T> struct A::TZ {};
namespace B { export struct B::W {}; }
