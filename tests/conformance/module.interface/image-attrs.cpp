// [dcl.attr.nodiscard]/4, [dcl.attr.deprecated]/4, spec/modules.md §11.11
// "Attributes" (WP-1278): an importer of `imga` warns about uses of its
// nodiscard and deprecated declarations (their notes in the module's
// header) exactly as after a replay, also when it builds `imga` from its
// declaration image (`--bmi-image`).
//@ expect: compile-ok
//@ std: c++20
//@ module: imga=Inputs/img-attrs.cppm
//@ two-step
//@ strict-diags
//@ diag: 16:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 17:3 warning "declared with 'nodiscard' attribute"
//@ diag: 18:15 warning "'old' is deprecated: use must"
import imga;
int use() {
  ia::must(1);
  ia::make_res();
  int r = ia::old(2);
  return r + ia::must(2);
}
