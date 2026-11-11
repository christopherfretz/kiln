// [temp.constr.constr], [temp.constr.atomic], [expr.prim.req],
// [module.interface], spec/modules.md §11.11 "Constraints" (WP-1284): an
// importer's errors against constrained templates from the declaration
// image are the ones against the replayed source, with the same notes
// on the unsatisfied constraints in the module (pinned in
// crates/kiln_driver/tests/modules_bmi.rs).
//@ expect: compile-fail
//@ std: c++20
//@ module: imgc=Inputs/img-constraints.cppm
//@ strict-diags
//@ diag: 17:9 error "no matching function for call to 'pick'"
//@ diag: 18:5 error "constraints not satisfied for alias template 'BigAlias' [with T = char]"
//@ diag: 19:26 error "no matching member function for call to 'h'"
//@ diag: 20:9 error "no matching function for call to 'f'"
import imgc;
struct Three { char c[3]; };
int x = ic::pick(Three{});
ic::BigAlias<char> y;
int z = ic::Box<Three>().h();
int w = ic::f(1);
