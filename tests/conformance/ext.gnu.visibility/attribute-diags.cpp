// The GNU `visibility` / `type_visibility` attributes ([dcl.attr.grammar]/5
// GNU extension; spec/sema.md "Visibility attributes", clang
// handleVisibilityAttr / mergeVisibilityAttr): the argument must be one
// string literal naming default, hidden, internal or protected (an unknown
// one is ignored with a warning); redeclarations must agree, an attribute
// added after the definition is ignored, and the attribute is ignored on a
// typedef. `type_visibility` applies only to types and namespaces.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:27 warning "'visibility' attribute argument not supported: bogus"
//@ diag: 24:27 error "expected string literal as argument of 'visibility' attribute"
//@ diag: 25:16 error "'visibility' attribute takes one argument"
//@ diag: 26:16 error "'visibility' attribute takes one argument"
//@ diag: 28:16 error "visibility does not match previous declaration"
//@ diag: 27:16 note "previous attribute is here"
//@ diag: 29:16 error "visibility does not match previous declaration"
//@ diag: 29:38 note "previous attribute is here"
//@ diag: 30:30 warning "'visibility' attribute ignored"
//@ diag: 42:16 warning "attribute declaration must precede definition"
//@ diag: 41:6 note "previous definition is here"
//@ diag: 47:16 error "'type_visibility' attribute only applies to types and namespaces"
//@ diag: 48:39 warning "'type_visibility' attribute argument not supported: bogus"
__attribute__((visibility("bogus"))) void f1();
__attribute__((visibility(1))) void f2();
__attribute__((visibility)) void f3();
__attribute__((visibility("hidden","default"))) void f4();
__attribute__((visibility("hidden"))) void f5();
__attribute__((visibility("default"))) void f5();
__attribute__((visibility("hidden"), visibility("default"))) void f6();
typedef int T __attribute__((visibility("hidden")));
[[gnu::visibility("hidden")]] int v1;
struct __attribute__((visibility("hidden"))) S {
  int m __attribute__((visibility("default")));
};
void g() { __attribute__((visibility("hidden"))) int loc; static int sl __attribute__((visibility("hidden"))); }
enum __attribute__((visibility("hidden"))) E { e0 };
namespace __attribute__((visibility("hidden"))) N { }
namespace [[gnu::visibility("default")]] N2 { }
void f7();
__attribute__((visibility("hidden"))) void f7();
void f8() {}
__attribute__((visibility("hidden"))) void f8();
__attribute__((visibility("internal"))) void f9();
__attribute__((visibility("protected"))) void f10();
static __attribute__((visibility("hidden"))) void f11() {}
namespace { __attribute__((visibility("hidden"))) void f12() {} }
__attribute__((type_visibility("hidden"))) void f13();
struct __attribute__((type_visibility("bogus"))) S2 {};
struct __attribute__((type_visibility("default"),visibility("hidden"))) S3 {};
