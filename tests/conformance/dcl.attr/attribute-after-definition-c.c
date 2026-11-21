// [clang ext] "attribute declaration must precede definition" in C
// (clang `Sema::checkNewAttributesAfterDef`): a tentative definition
// (C17 6.9.2) is not the definition there, so an attribute added after it
// is merged; one after a definition with an initializer, a function
// body or a struct body is ignored with the warning. C11 `_Noreturn` is
// exempt. spec/sema.md §4.5, WP-1807.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 20:16 warning "attribute declaration must precede definition"
//@ diag: 19:5 note "previous definition is here"
//@ diag: 23:30 warning "attribute declaration must precede definition"
//@ diag: 22:5 note "previous definition is here"
//@ diag: 25:16 warning "attribute declaration must precede definition"
//@ diag: 24:6 note "previous definition is here"
//@ diag: 27:23 warning "attribute declaration must precede definition"
//@ diag: 26:8 note "previous definition is here"
int t1;
__attribute__((used)) int t1;
int t2 = 1;
__attribute__((used)) int t2;
extern int t3;
int t3 = 2;
extern int t3 __attribute__((weak));
void f(void) {}
__attribute__((cold)) void f(void);
struct S { int x; };
struct __attribute__((packed)) S;
_Noreturn void g(void) { for (;;) {} }
_Noreturn void g(void);
