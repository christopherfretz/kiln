// [class.access]/1, [temp.friend]/1, [namespace.memdef]/3,
// [module.interface], spec/modules.md §11.11 "Friends" (WP-1307): an
// importer's errors against friend templates and friends of class
// templates built from the declaration image are the ones against the
// replayed source: a private member named outside the class and its
// friends, and hidden friends named by a qualified name.
//@ expect: compile-fail
//@ std: c++20
//@ module: imgft=Inputs/img-friend-template.cppm
//@ diag: 15:35 error "'secret' is a private member of '"
//@ diag: 16:36 error "no member named 'peek' in namespace 'ft'"
//@ diag: 17:30 error "no member named 'f' in namespace 'ft'"
//@ diag: 19:43 error "'p' is a private member of '"
import imgft;
int a() { return ft::Holder<int>::secret; }
int b(ft::It<int> &i) { return ft::peek(i); }
int c(ft::A &x) { return ft::f(1, x); }
struct Q {
  int *h(const ft::It<int> &i) { return i.p; }
};
