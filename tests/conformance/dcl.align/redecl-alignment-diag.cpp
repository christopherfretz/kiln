// [dcl.align]/6: if any declaration of an entity has an alignment-specifier,
// every defining declaration must specify an equivalent alignment, and two
// declarations with alignas must agree (clang `mergeAlignedAttrs`): a
// definition without alignas after a declaration with one ("'alignas' must
// be specified on definition..." at the definition, noting the inherited
// alignas), different alignments ("redeclaration has different alignment
// requirement (new vs old)"; alignas(0) means the natural alignment), and
// an alignas added after a definition without one (clang
// `checkNewAttributesAfterDef`). Redeclarations without alignas inherit it.
// Variables, static data members, classes, unions and class templates.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 41:5 error "'alignas' must be specified on definition if it is specified on any declaration"
//@ diag: 39:1 note "declared with 'alignas' attribute here"
//@ diag: 45:1 error "redeclaration has different alignment requirement (16 vs 8)"
//@ diag: 44:1 note "previous declaration is here"
//@ diag: 51:8 error "'alignas' must be specified on definition if it is specified on any declaration"
//@ diag: 50:8 note "declared with 'alignas' attribute here"
//@ diag: 55:8 error "redeclaration has different alignment requirement (16 vs 8)"
//@ diag: 54:8 note "previous declaration is here"
//@ diag: 60:8 error "redeclaration has different alignment requirement (16 vs 8)"
//@ diag: 59:8 note "previous declaration is here"
//@ diag: 61:8 error "'alignas' must be specified on definition if it is specified on any declaration"
//@ diag: 62:8 note "declared with 'alignas' attribute here"
//@ diag: 65:5 error "'alignas' must be specified on definition if it is specified on any declaration"
//@ diag: 63:1 note "declared with 'alignas' attribute here"
//@ diag: 69:8 error "'alignas' must be specified on definition if it is specified on any declaration"
//@ diag: 68:12 note "declared with 'alignas' attribute here"
//@ diag: 71:1 error "redeclaration has different alignment requirement (4 vs 8)"
//@ diag: 70:1 note "previous declaration is here"
//@ diag: 72:5 error "'alignas' must be specified on definition if it is specified on any declaration"
//@ diag: 71:1 note "declared with 'alignas' attribute here"
//@ diag: 74:26 error "'alignas' must be specified on definition if it is specified on any declaration"
//@ diag: 73:26 note "declared with 'alignas' attribute here"
//@ diag: 80:5 error "'alignas' must be specified on definition if it is specified on any declaration"
//@ diag: 81:1 note "declared with 'alignas' attribute here"
//@ diag: 83:7 error "redeclaration has different alignment requirement (16 vs 8)"
//@ diag: 82:7 note "previous declaration is here"
alignas(8) extern int v1;
extern int v1;
int v1;
extern int v2;
alignas(8) int v2;
alignas(8) extern int v3;
alignas(16) int v3;
alignas(8) extern int v4;
alignas(8) int v4;
alignas(8) extern int v5;
alignas(4) alignas(8) int v5;
struct alignas(8) C1;
struct C1 { int i; };
struct C2;
struct alignas(8) C2 { int i; };
struct alignas(8) C3;
struct alignas(16) C3 { int i; };
struct alignas(8) C4;
struct C4;
struct alignas(8) C4 { int i; };
struct alignas(8) C5 { int i; };
struct alignas(16) C5;
struct C6 { int i; };
struct alignas(8) C6;
alignas(8) extern int v7;
extern int v7 __attribute__((aligned(8)));
int v7;
struct X { static int m; };
alignas(8) int X::m;
struct Y { alignas(8) static int m; };
int Y::m;
alignas(8) extern int v8;
alignas(int) extern int v8;
int v8;
template<class T> struct alignas(8) C9;
template<class T> struct C9 { };
alignas(8) extern int v10;
extern int v10;
alignas(0) extern int v11;
alignas(int) extern int v11;
alignas(int) int v11;
int v12;
alignas(8) extern int v12;
union alignas(8) U13;
union alignas(16) U13 { int i; };
