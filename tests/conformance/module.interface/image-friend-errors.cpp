// [class.access]/1, [class.friend]/7, [namespace.memdef]/3,
// [module.interface], spec/modules.md §11.11 "Friends" (WP-1292): an
// importer's errors against friends built from the declaration image are
// the ones against the replayed source: a private member named outside
// the class and its friends, and a hidden friend named by a qualified
// name.
//@ expect: compile-fail
//@ std: c++20
//@ module: imgfr=Inputs/img-friend.cppm
//@ diag: 14:30 error "'v' is a private member of '"
//@ diag: 15:32 error "no member named 'twice' in namespace 'fr'"
//@ diag: 17:32 error "'v' is a private member of '"
import imgfr;
int f(fr::Box &b) { return b.v; }
int g(fr::Box &b) { return fr::twice(b); }
struct Q {
  int h(fr::Box &b) { return b.v; }
};
