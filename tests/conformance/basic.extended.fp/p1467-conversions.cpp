// [conv.double]/2, [conv.rank]/2, [expr.arith.conv]/1.4, [dcl.init.list]/7.2,
// [expr.cond]/7.3 (C++23, P1467R9): an implicit conversion to a floating type
// whose conversion rank is less than the source's (or unordered with it) is
// not a standard conversion; kiln, like GCC 14, accepts it with a warning
// (-Wfloat-conversion-rank, on by default; GCC's group is -Wnarrowing).
// Mixed arithmetic, ?: and compound assignment between operands of
// unordered rank (_Float16 and __bf16) are errors. List-initialization from
// a type of greater rank is narrowing unless the value is a constant
// expression that fits exactly. Diagnostic lines and columns match GCC 14.
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ diag: 44:17 warning "ISO C++ does not allow converting to '_Float32' from 'double' with greater conversion rank"
//@ diag: 45:17 warning "ISO C++ does not allow converting to '_Float16' from 'float' with greater conversion rank"
//@ diag: 46:15 warning "ISO C++ does not allow converting to '__bf16' from '_Float16' with unordered conversion rank"
//@ diag: 47:17 warning "ISO C++ does not allow converting to '_Float16' from '__bf16' with unordered conversion rank"
//@ diag: 48:17 warning "ISO C++ does not allow converting to '_Float32' from '_Float64' with greater conversion rank"
//@ diag: 49:17 warning "ISO C++ does not allow converting to '_Float32' from 'double' with greater conversion rank"
//@ diag: 51:15 error "invalid arithmetic between floating-point types '_Float16' and '__bf16' with unordered conversion ranks"
//@ diag: 52:15 error "invalid conditional expression between floating-point types '_Float16' and '__bf16' with unordered conversion ranks"
//@ diag: 53:5 error "invalid compound assignment to floating-point types '_Float16' and '__bf16' with unordered conversion ranks"
//@ diag: 54:15 error "non-constant-expression cannot be narrowed from type 'double' to '_Float32' in initializer list"
//@ diag: 54:15 note "insert an explicit cast to silence this issue"
//@ diag: 54:15 warning "ISO C++ does not allow converting to '_Float32' from 'double' with greater conversion rank"
//@ diag: 55:15 warning "ISO C++ does not allow converting to '_Float16' from 'double' with greater conversion rank"
//@ xfail(clang): neither clang 19 nor clang 22 implements P1467 (no _Float32/_Float64, no rank rule)
_Float16 h; __bf16 b; _Float32 f32; _Float64 f64; _Float128 f128;
float f; double d; long double ld;

void ok() {
  float a1 = f32;         // equal rank: fine
  double a2 = f32;        // greater rank: fine
  _Float64 a3 = d;        // equal rank: fine
  _Float128 a4 = ld;      // _Float128's rank is >= long double's
  float a5 = h; float a6 = b;
  (void)a1; (void)a2; (void)a3; (void)a4; (void)a5; (void)a6;
  _Float32 i1{f};         // same rank, not narrowing
  _Float64 i2{f32};       // greater rank, not narrowing
  (void)i1; (void)i2;
  h = h + 1; b += 1.0bf16; f32 *= f;
}

void bad() {
  _Float32 e1 = d;
  _Float16 e2 = f;
  __bf16 e3 = h;
  _Float16 e4 = b;
  _Float32 e5 = f64;
  _Float32 e6 = 1.5;      // a constant of greater rank is still diagnosed
  (void)e1; (void)e2; (void)e3; (void)e4; (void)e5; (void)e6;
  auto u1 = h + b;
  auto u2 = d ? h : b;
  h += b;
  _Float32 n1{d};         // narrowing (GCC 14: only the rank warning)
  _Float16 n2{0.1};       // a constant within range is not narrowing (7.2)
  (void)u1; (void)u2; (void)n1; (void)n2;
}
