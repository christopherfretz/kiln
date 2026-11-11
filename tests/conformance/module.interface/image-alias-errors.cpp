// [temp.deduct.type]/8, [temp.arg]/1, [temp.alias], [module.interface],
// spec/modules.md §11.11 "Templates as patterns" (WP-1279): an importer's
// errors against alias templates built from the declaration image are the
// ones against the replayed source (the errors inside an alias's
// substitution, at its `using` keyword in the module, are pinned in
// crates/kiln_driver/tests/modules_bmi.rs).
//@ expect: compile-fail
//@ std: c++20
//@ module: imga=Inputs/img-alias.cppm
//@ strict-diags
//@ diag: 15:9 error "no matching function for call to 'unbox'"
//@ diag: 16:5 error "too many template arguments for alias template 'Fn'"
//@ diag: 17:9 error "no matching function for call to 'deref'"
import imga;
int x = ia::unbox(3);
ia::Fn<int, int> f;
int y = ia::deref(3);
