// [expr.const]/10, [cpp.predefined], [module.interface], spec/modules.md
// §11.11 (WP-1310): an overflow in an imported function template whose
// operand is a predefined macro makes a static assertion's condition not
// a constant expression, under `--bmi-image` as with the replayed source.
//@ expect: compile-fail
//@ std: c++20
//@ module: imgpa=Inputs/img-predef-aligned.cppm
//@ diag: 11:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:15 note "in call to 'add<int>(1)'"
import imgpa;
static_assert(pa::add(1) > 0);

char c = pa::trunc<char>();
