// GNU selectany (WP-809; clang SelectAnyAttr): an error on a function or on
// a variable without external linkage, ignored when added after the
// definition, no arguments; no effect on a type, a typedef or a field.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:23 error "'selectany' can only be applied to data items with external linkage"
//@ diag: 23:28 error "'selectany' can only be applied to data items with external linkage"
//@ diag: 24:29 error "'selectany' can only be applied to data items with external linkage"
//@ diag: 25:33 error "'selectany' can only be applied to data items with external linkage"
//@ diag: 26:16 error "'selectany' can only be applied to data items with external linkage"
//@ diag: 27:37 error "'selectany' can only be applied to data items with external linkage"
//@ diag: 27:82 error "'selectany' can only be applied to data items with external linkage"
//@ diag: 29:18 error "'selectany' can only be applied to data items with external linkage"
//@ diag: 30:18 error "'selectany' can only be applied to data items with external linkage"
//@ diag: 35:16 warning "attribute declaration must precede definition"
//@ diag: 34:5 note "previous definition is here"
//@ diag: 37:16 warning "attribute declaration must precede definition"
//@ diag: 36:12 note "previous definition is here"
//@ diag: 39:16 error "attribute takes no arguments"
//@ diag: 40:3 error "attribute takes no arguments"
// Not data with external linkage.
static __attribute__((selectany)) int s1 = 1;
namespace { __attribute__((selectany)) int s2 = 1; }
const int s3 __attribute__((selectany)) = 1;
constexpr int s4 __attribute__((selectany)) = 1;
__attribute__((selectany)) void f1();
struct M { void f2() __attribute__((selectany)); static void f3() __attribute__((selectany)); };
void g() {
  __attribute__((selectany)) static int s5 = 1;
  __attribute__((selectany)) int s6 = 1;
  (void)s5; (void)s6;
}
// Added after the definition: ignored.
int d1 = 1;
__attribute__((selectany)) extern int d1;
static int d2;
__attribute__((selectany)) extern int d2;
// Arguments.
__attribute__((selectany(1))) int a1 = 1;
[[gnu::selectany(1)]] int a2 = 1;
// Accepted.
__attribute__((selectany)) int ok1 = 1;
extern int ok2;
__attribute__((selectany)) int ok2 = 2;
struct __attribute__((selectany)) T { int m __attribute__((selectany)); };
typedef int TI __attribute__((selectany));
