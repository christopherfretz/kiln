// [dcl.enum]/5, [temp.inst]/3, [module.interface], spec/modules.md
// §11.11 (WP-1310): an enumerator of an imported class template has the
// value its instantiation gives it, and an initializer invalid for the
// arguments is diagnosed at the instantiation, under `--bmi-image` as
// with the replayed source.
//@ expect: compile-fail
//@ std: c++20
//@ module: imgme=Inputs/img-member-enums.cppm
//@ diag: 12:15 error "static assertion failed due to requirement 'me::A<char>::d == 5'"
//@ diag: 12:30 note "expression evaluates to '4 == 5'"
import imgme;
static_assert(me::A<char>::d == 5);
me::B<int> k;
