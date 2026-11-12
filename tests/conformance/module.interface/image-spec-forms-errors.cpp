// [temp.spec.partial.match]/2, [temp.mem]/1, [module.interface],
// spec/modules.md §11.11 "Forms carried by WP-1358": an importer's
// static assertions see the module's constrained partial
// specializations and variable template partial specializations, from
// the declaration image as from the replayed source.
//@ expect: compile-fail
//@ std: c++20
//@ module: imgsf=Inputs/img-spec-forms.cppm
//@ diag: 12:15 error "static assertion failed"
//@ diag: 13:15 error "static assertion failed"
import imgsf;
static_assert(sf::A<double *>::k == 3);
static_assert(sf::k_v<long *> == 1);
static_assert(sf::A<int>::k == 1 && sf::has<int>::value);
