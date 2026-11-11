// [temp.arg]/1, [temp.names]/8, [module.interface], spec/modules.md
// §11.11 "Templates as patterns" (WP-1279): an importer's errors against
// variable templates built from the declaration image are the ones
// against the replayed source (errors inside an instantiated initializer,
// located in the module, are pinned in
// crates/kiln_driver/tests/modules_bmi.rs).
//@ expect: compile-fail
//@ std: c++20
//@ module: imgv=Inputs/img-var.cppm
//@ strict-diags
//@ diag: 15:13 error "too many template arguments for variable template 'sq'"
//@ diag: 16:13 error "use of variable template 'iv::is_int' requires template arguments"
import imgv;
struct S {};
int c = iv::sq<1, 2>;
int e = iv::is_int;
