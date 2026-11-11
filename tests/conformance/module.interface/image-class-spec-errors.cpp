// [temp.spec.partial.match], [temp.expl.spec]/1, [temp.inst]/1,
// [dcl.array]/1, [module.interface], spec/modules.md §11.11 "Templates as
// patterns" (WP-1283): an importer's errors against partial and explicit
// specializations built from the declaration image are the ones against
// the replayed source: a declared but undefined explicit specialization
// and a primary template that is never defined (an error inside an
// imported partial specialization's pattern, at its `[` in the module,
// is pinned in crates/kiln_driver/tests/modules_bmi.rs).
//@ expect: compile-fail
//@ std: c++20
//@ module: imgcs=Inputs/img-cspec.cppm
//@ strict-diags
//@ diag: 16:13 error "variable has incomplete type 'cs::B<void>'"
//@ diag: 17:12 error "implicit instantiation of undefined template 'cs::B<int>'"
import imgcs;
cs::B<void> b;
cs::B<int> c;
