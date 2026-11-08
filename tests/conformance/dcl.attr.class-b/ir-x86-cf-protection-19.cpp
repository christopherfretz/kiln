// [driver] `-fcf-protection=full` on x86 gives the module flags
// "cf-protection-branch" / "cf-protection-return" (CodeGenModule::Release)
// and a `nocf_check` function gets the `nocf_check` attribute (no
// ENDBR64); the others, and their indirect calls, are unchanged
// (spec/landed/codegen/WP-813.md). WP-813.
//@ requires: clang<22
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -fcf-protection=full
void g();
__attribute__((nocf_check)) void nocf() { g(); }
void plain() { g(); }
void call(void (*p)()) { p(); }
