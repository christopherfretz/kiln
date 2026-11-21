// clang `handleWeakImportAttr` and `checkNewAttributesAfterDef`:
// `weak_import` on a definition (a variable defined here, a parameter)
// or after the entity's definition is ignored with a warning, as it is
// on anything but a variable or function. A
// weak imported variable's address is not a constant, and testing it is
// no tautology. spec/sema.md §4.5, WP-1745.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:23 warning "'weak_import' attribute cannot be specified on a definition"
//@ diag: 23:29 warning "'weak_import' attribute cannot be specified on a definition"
//@ diag: 24:29 warning "'weak_import' attribute cannot be specified on a definition"
//@ diag: 26:25 warning "attribute declaration must precede definition"
//@ diag: 25:6 note "previous definition is here"
//@ diag: 28:29 warning "attribute declaration must precede definition"
//@ diag: 27:5 note "previous definition is here"
//@ diag: 29:23 warning "'weak_import' attribute only applies to variables and functions"
//@ diag: 30:26 warning "'weak_import' attribute only applies to variables and functions"
//@ diag: 31:21 warning "'weak_import' attribute only applies to variables and functions"
//@ diag: 32:33 warning "'weak_import' attribute only applies to variables and functions"
//@ diag: 34:16 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 34:23 note "comparison against address of weak declaration '&w' can only be performed at runtime"
int b2 __attribute__((weak_import));
static int s __attribute__((weak_import));
void k(int p __attribute__((weak_import)));
void g() {}
void g() __attribute__((weak_import));
int e;
extern int e __attribute__((weak_import));
struct __attribute__((weak_import)) T {};
namespace __attribute__((weak_import)) ns {}
enum __attribute__((weak_import)) E { e0 };
struct U { int f __attribute__((weak_import)); };
extern int w __attribute__((weak_import));
constexpr bool b = &w != nullptr;
bool t() { return &w; }
