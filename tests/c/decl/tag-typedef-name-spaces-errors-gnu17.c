// C17 6.2.3p1: with tags and ordinary identifiers in separate name spaces,
// the C diagnostics that remain are within one name space: a tag
// redefined, a tag redeclared with another keyword, a typedef redefined
// with a different type; a typedef of the same name never suppresses or
// changes them (wording and locations as clang 19). WP-1427.
//@ args: -std=gnu17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:8 error "redefinition of 'r'"
//@ diag: 23:8 note "previous definition is here"
//@ diag: 26:1 error "use of 'k' with tag type that does not match previous declaration"
//@ diag: 25:8 note "previous use is here"
//@ diag: 28:14 error "typedef redefinition with different types ('long' vs 'int')"
//@ diag: 27:13 note "previous definition is here"
//@ diag: 31:1 error "use of 'w' with tag type that does not match previous declaration"
//@ diag: 30:8 note "previous use is here"
//@ diag: 34:7 error "redefinition of 'x'"
//@ diag: 33:7 note "previous definition is here"
//@ diag: 38:3 error "use of 'y' with tag type that does not match previous declaration"
//@ diag: 37:8 note "previous use is here"
//@ diag: 39:17 error "typedef redefinition with different types ('float' vs 'int')"
//@ diag: 36:15 note "previous definition is here"
struct r { int a; };
struct r { int b; };
struct k { int a; };
union k;
typedef int td;
typedef long td;
typedef int w;
struct w { int m; };
enum w *ew;
typedef char x;
union x { int i; };
union x { int j; };
void blk(void) {
  typedef int y;
  enum y { Y0 };
  struct y *sy;
  typedef float y;
}
