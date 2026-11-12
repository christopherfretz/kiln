// [dcl.pre]/10-11, [temp.inst]/4, [module.interface], spec/modules.md
// §11.11 (WP-1314): an imported function template's block-scope static
// assertion that fails for an instantiation's arguments is diagnosed at
// that instantiation, under `--bmi-image` as with the replayed source.
//@ expect: compile-fail
//@ diag: 14:9 error "no matching function for call to 'pick'"
//@ std: c++20
//@ module: imglsa=Inputs/img-local-static-assert.cppm
import imglsa;
struct Big { char c[100]; operator int() const { return 0; } };
struct Three { char c[3]; };
int a = lsa::f(Big{});
int b = lsa::S<Three>::h();
int c = lsa::pick<int>(nullptr);
