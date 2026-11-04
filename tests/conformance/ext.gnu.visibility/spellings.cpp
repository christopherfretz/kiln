// The `visibility` attribute's spellings ([dcl.attr.grammar]/5 GNU
// extension; spec/sema.md "Visibility attributes"): `__attribute__` and
// `[[gnu::]]`, each with or without the reserved `__visibility__` form,
// are one attribute, so redeclarations compare across spellings.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:3 error "visibility does not match previous declaration"
//@ diag: 13:3 note "previous attribute is here"
//@ diag: 17:23 error "visibility does not match previous declaration"
//@ diag: 16:10 note "previous attribute is here"
[[gnu::__visibility__("hidden")]] void k1();
__attribute__((__visibility__("hidden"))) void k1();
[[gnu::visibility("hidden")]] void k1();
[[gnu::visibility("default")]] void k1();
namespace [[gnu::visibility("hidden")]] N {}
struct [[gnu::visibility("hidden")]] S;
struct __attribute__((visibility("default"))) S {};
