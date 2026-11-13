// [dcl.fct]/13, [over.dcl], [module.interface], spec/modules.md §11.11
// "Forms carried by WP-1369": an importer's static assertion sees a
// function declared through a noexcept typedef as non-throwing, and a
// function declared through a typedef as returning the typedef's return
// type, from the declaration image as from the replayed source.
//@ expect: compile-fail
//@ std: c++20
//@ module: imgtf=Inputs/img-typedef-fns.cppm
//@ diag: 12:15 error "static assertion failed"
//@ diag: 13:15 error "static assertion failed"
import imgtf;
static_assert(!noexcept(tf::thrice(1)));
static_assert(sizeof(tf::twice(1)) == sizeof(long));
static_assert(noexcept(tf::Box<int>::quad(1)));
