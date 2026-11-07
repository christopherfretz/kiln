// GCC "Common Function Attributes", alias / ifunc (clang handleAliasAttr,
// handleIFuncAttr, Sema::CheckCompleteVariableDeclaration,
// ActOnStartOfFunctionDef and MergeFunctionDecl / MergeVarDecl): `alias`
// and `ifunc` take exactly one string literal (the target's mangled name)
// and make the declaration a definition, so neither can sit on a function
// body or a variable definition, nor appear twice, nor be followed (or
// preceded) by another definition of the entity. `ifunc` on a variable is
// ignored with -Wignored-attributes. The target checks (defined, no
// cycle, resolver returns a pointer) run at code generation:
// crates/kiln_driver/tests/codegen.rs alias_ifunc_diagnostics.
// Not checked: clang's -Wgcc-compat "GCC does not allow 'ifunc' attribute
// in this position on a function definition" (kiln has no -Wgcc-compat);
// the definition below writes its attribute in prefix position.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:23 warning "'ifunc' attribute only applies to functions"
//@ diag: 39:16 error "definition 'f0' cannot also be an ifunc"
//@ diag: 40:42 error "definition 'f1' cannot also be an alias"
//@ diag: 41:43 error "definition 'f2' cannot also be an ifunc"
//@ diag: 42:43 error "definition 'f3' cannot also be an alias"
//@ diag: 43:28 error "'ifunc' attribute takes one argument"
//@ diag: 44:34 error "expected string literal as argument of 'ifunc' attribute"
//@ diag: 45:28 error "'ifunc' attribute takes one argument"
//@ diag: 47:5 error "redefinition of 'f7'"
//@ diag: 46:5 note "previous definition is here"
//@ diag: 49:5 error "redefinition of 'f8'"
//@ diag: 48:5 note "previous definition is here"
//@ diag: 51:5 error "redefinition of 'v1'"
//@ diag: 50:12 note "previous definition is here"
//@ diag: 53:12 error "redefinition of 'v2'"
//@ diag: 52:5 note "previous definition is here"
//@ diag: 54:23 error "definition 'v3' cannot also be an alias"
//@ diag: 55:42 error "definition 'v4' cannot also be an alias"
extern "C" int impl(int x) { return x; }
extern "C" void *res() { return (void *)impl; }
extern "C" int g = 1;

int v0 __attribute__((ifunc("res")));                   // not a function
__attribute__((ifunc("res"))) int f0(int) { return 0; } // a definition
int f1(int) __attribute__((ifunc("res"), alias("impl")));
int f2(int) __attribute__((alias("impl"), ifunc("res")));
int f3(int) __attribute__((alias("impl"), alias("impl")));
int f4(int) __attribute__((ifunc));
int f5(int) __attribute__((ifunc(res)));
int f6(int) __attribute__((ifunc("res", "x")));
int f7(int) __attribute__((ifunc("res")));
int f7(int) { return 1; }
int f8(int) { return 0; }
int f8(int) __attribute__((alias("impl")));
extern int v1 __attribute__((alias("g")));
int v1 = 2;
int v2;
extern int v2 __attribute__((alias("g")));
int v3 __attribute__((alias("g")));                     // a definition
extern int v4 __attribute__((alias("g"), alias("g")));

// Accepted: declarations only.
int ok1(int) __attribute__((ifunc("res")));
[[gnu::ifunc("res")]] int ok2(int);
[[gnu::alias("impl")]] int ok3(int);
int ok3(int);
extern int ok4 __attribute__((alias("g")));
static int ok5 __attribute__((alias("g")));
