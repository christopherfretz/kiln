// [module.interface]/2, [basic.lookup.qual]/1, spec/modules.md §11.11
// "Part 2a": an importer's errors against declarations built from the
// declaration image are the ones against the replayed source.
//@ expect: compile-fail
//@ std: c++20
//@ module: img=Inputs/img.cppm
//@ two-step
//@ min-oracle: 22
//@ strict-diags
//@ diag: 14:12 error "cannot initialize a variable of type 'geo::Color' with an rvalue of type 'int'"
//@ diag: 15:14 error "no member named 'nothere' in namespace 'geo'"
//@ diag: 16:9 error "use of undeclared identifier 'helper'"
import img;
geo::Color c = 3;
int y = geo::nothere;
int z = helper(1);
