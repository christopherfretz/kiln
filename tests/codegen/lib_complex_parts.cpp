// clang EmitBuiltinExpr (WP-1073): creal / crealf, cimag / cimagf and
// conj / conjf are library builtins lowered inline: the real or imaginary
// part of the operand, or the operand with its imaginary part negated
// (constant-folded for a constant operand).
extern "C" {
double creal(_Complex double);
float crealf(_Complex float);
double cimag(_Complex double);
float cimagf(_Complex float);
_Complex double conj(_Complex double);
_Complex float conjf(_Complex float);
}
double f1(_Complex double z) { return creal(z) + cimag(z); }
float f2(_Complex float z) { return crealf(z) - cimagf(z); }
_Complex double f3(_Complex double z) { return conj(z); }
_Complex float f4(_Complex float z) { return conjf(z); }
double f5() { return creal(1.0 + 2.0i) + cimag(conj(3.0 + 4.0i)); }
double (*p)(_Complex double) = creal;
