// [WP-173] Constant-evaluated vector values ([expr.const]; clang
// ConstExprEmitter / APValue::Vector): constexpr and constant-initialized
// globals of GNU and ext vector type are emitted as folded `<N x T>`
// constants, and constant vector expressions inside functions fold to
// stored constants.
typedef int v4i __attribute__((ext_vector_type(4)));
typedef float v4f __attribute__((ext_vector_type(4)));
typedef int v3i __attribute__((ext_vector_type(3)));
typedef int g4i __attribute__((vector_size(16)));
typedef short g8s __attribute__((vector_size(16)));
typedef double g2d __attribute__((vector_size(16)));
typedef unsigned char g8c __attribute__((vector_size(8)));

constexpr g4i a = {1, 2, 3, 4};
constexpr g4i b = a * 3 + 1;
constexpr g4i m = a > 2;
constexpr v4i s = (v4i)7 - a;
constexpr v4f f = __builtin_convertvector(a, v4f) / 2;
constexpr g8s bc = (g8s)a;
constexpr g2d d = {1.5, -0.25};
constexpr v3i three = {9, 8, 7};
constexpr g8c sh0 = {1, 2, 3, 4, 5, 6, 7, 8};
constexpr g8c sh = sh0 << 1;
g4i zero = {};
g4i c = a ^ ~a;

constexpr g4i twice(g4i x) {
  g4i r = x;
  r += x;
  return r;
}
g4i t = twice(b);

const g4i* use() {
  static const g4i *p[] = {&a, &b, &m, &bc ? &a : &b};
  return p[0];
}
v4f usef() { return f; }
v4i uses() { return s; }
g2d used() { return d; }
v3i use3() { return three; }
g8c usesh() { return sh; }
int local() {
  constexpr g4i l = a - b;
  return __builtin_reduce_add(l);
}
