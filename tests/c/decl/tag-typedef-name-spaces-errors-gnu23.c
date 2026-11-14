// C23 6.2.3p1: with tags and ordinary identifiers in separate name spaces,
// the C diagnostics that remain are within one name space: a tag
// redeclared with another keyword, a typedef redefined
// with a different type; a typedef of the same name never suppresses or
// changes them (wording and locations as clang 19). A redefined tag is
// tag-typedef-name-spaces-errors-gnu17.c's: C23 6.7.2.3p1 (N3037) lets a
// tag be redefined compatibly, which clang 22 words differently. WP-1427.
//@ args: -std=gnu23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:1 error "use of 'k' with tag type that does not match previous declaration"
//@ diag: 21:8 note "previous use is here"
//@ diag: 24:14 error "typedef redefinition with different types ('long' vs 'int')"
//@ diag: 23:13 note "previous definition is here"
//@ diag: 27:1 error "use of 'w' with tag type that does not match previous declaration"
//@ diag: 26:8 note "previous use is here"
//@ diag: 31:3 error "use of 'y' with tag type that does not match previous declaration"
//@ diag: 30:8 note "previous use is here"
//@ diag: 32:17 error "typedef redefinition with different types ('float' vs 'int')"
//@ diag: 29:15 note "previous definition is here"
struct k { int a; };
union k;
typedef int td;
typedef long td;
typedef int w;
struct w { int m; };
enum w *ew;
void blk(void) {
  typedef int y;
  enum y { Y0 };
  struct y *sy;
  typedef float y;
}
