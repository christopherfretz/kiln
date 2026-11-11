// [temp.mem]/1, [temp.arg.general]/1, [temp.deduct.call]/1,
// [module.interface], spec/modules.md §11.11 "Member templates"
// (WP-1292): an importer's errors against member templates built from
// the declaration image are the ones against the replayed source: a
// member class template named with too few arguments, and a member
// function template that cannot be deduced (an error inside an imported
// member template's pattern, at its `.` in the module, is pinned in
// crates/kiln_driver/tests/modules_bmi.rs).
//@ expect: compile-fail
//@ std: c++20
//@ module: imgmt=Inputs/img-membertempl.cppm
//@ strict-diags
//@ diag: 16:8 error "too few template arguments for class template 'In'"
//@ diag: 17:9 error "no matching function for call to 'g'"
import imgmt;
mt::A::In<> x;
int y = mt::A::g();
