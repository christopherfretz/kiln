// A braced initializer of a vector type whose element is a vector of
// that type (sema keeps it as the whole initializer) lowers as clang's
// `VisitInitListExpr`: the element widened to the result's length by one
// `shufflevector`, then shuffled in after the lanes already set; scalar
// lanes stay `insertelement`s (a JSON library's SIMD UTF-8 validator,
// `vec m{zero()}` default member initializers; WP-792).
//@ expect: ir
//@ requires: clang<22
//@ args: -ftrivial-auto-var-init=uninitialized
typedef unsigned char v16 __attribute__((vector_size(16)));
v16 zero() { return v16{}; }
struct C {
  v16 e{zero()};
  v16 f = {zero()};
};
void copy(v16 *out, const v16 *in) {
  v16 b{*in};
  C c{};
  v16 d = {b};
  *out = b + c.e + c.f + d;
}
v16 mixed(unsigned char x) { return v16{x, 2}; }
