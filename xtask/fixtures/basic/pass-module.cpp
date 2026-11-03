//@ module: fx_a=Inputs/fx_a.cppm
//@ module: fx_b=Inputs/fx_b.cppm
//@ strict-diags
// WP-405: clang precompiles each `module:` unit in order (fx_b with fx_a's
// BMI) and compiles the test with `-fmodule-file=<name>=<bmi>`.
import fx_b;
int g() { return fb(); }
