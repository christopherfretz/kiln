// [cmath.syn]/2: 'float_control(precise, off)' turns errno off in its
// scope, so at -O1 clang lowers the library calls there to intrinsics
// (clang GenerateIntrinsics; spec/codegen.md "Floating-point
// semantics", WP-806).
//@ args: -O1 -Xclang -disable-llvm-passes
extern "C" { double sqrt(double); double pow(double, double); }
double fast(double x) {
#pragma float_control(precise, off)
  return sqrt(x) + pow(x, 3.0);
}
double strict(double x) { return sqrt(x) + pow(x, 3.0); }
