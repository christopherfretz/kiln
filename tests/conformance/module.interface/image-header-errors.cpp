// [module.interface]/2, [over.match.viable]/2, [basic.lookup.qual]/1,
// spec/modules.md §11.11 "File table" (stage S7): an importer's errors
// against declarations from a header #included in the purview of
// `imgh`, and against macro-made ones, are the ones against the
// replayed source (their notes in the header: modules_bmi.rs).
//@ expect: compile-fail
//@ std: c++20
//@ module: imgh=Inputs/img-hdr.cppm
//@ two-step
//@ min-oracle: 22
//@ strict-diags
//@ diag: 20:18 error "too few arguments to function call, expected 2, have 1"
//@ diag: 21:13 error "no member named 'nothere' in namespace 'hh'"
//@ diag: 22:23 error "too many arguments to function call, expected 0, have 1"
//@ diag: 23:25 error "no member named 'z' in 'hh::Point'"
//@ diag: 24:9 error "use of undeclared identifier 'IMGH_TWICE'"
//@ diag: 25:15 error "static assertion failed due to requirement 'hh::twice(2) == 5'"
//@ diag: 25:28 note "expression evaluates to '4 == 5'"
import imgh;
int a = hh::add(1);
int b = hh::nothere;
int c = hh::get_seven(1);
int d = hh::Point{1, 2}.z;
int e = IMGH_TWICE(1);
static_assert(hh::twice(2) == 5);
