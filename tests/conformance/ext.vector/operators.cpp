// Operators on vector operands (clang CheckVectorOperands, checkVectorShift,
// GetSignedVectorType): arithmetic, bitwise and shift operators keep the
// vector type and splat a scalar operand; comparisons and `!` yield the
// same-size signed integer vector (a bool vector for bool ext vectors);
// a subscript names an element lvalue (WP-156).
typedef int v4i __attribute__((ext_vector_type(4)));
typedef unsigned v4u __attribute__((ext_vector_type(4)));
typedef float v4f __attribute__((ext_vector_type(4)));
typedef double v2d __attribute__((ext_vector_type(2)));
typedef bool v4b __attribute__((ext_vector_type(4)));
typedef long v2l __attribute__((ext_vector_type(2)));
typedef short v8s __attribute__((ext_vector_type(8)));
typedef int g4i __attribute__((vector_size(16)));
typedef long long g2ll __attribute__((vector_size(16)));
typedef double g2d __attribute__((vector_size(16)));

v4i i, j;
v4u u;
v4f f;
v2d d;
v4b b;
v8s s;
g4i g;
g2d gd;

static_assert(__is_same(decltype(i + j), v4i));
static_assert(__is_same(decltype(i * 3), v4i));
static_assert(__is_same(decltype(5 - i), v4i));
static_assert(__is_same(decltype(f / 2.0f), v4f));
static_assert(__is_same(decltype(f * 2), v4f));
static_assert(__is_same(decltype(i % j), v4i));
static_assert(__is_same(decltype(i & j), v4i));
static_assert(__is_same(decltype(u ^ 1u), v4u));
static_assert(__is_same(decltype(i << 2), v4i));
static_assert(__is_same(decltype(i >> j), v4i));
static_assert(__is_same(decltype(s << 1), v8s));
static_assert(__is_same(decltype(~i), v4i));
static_assert(__is_same(decltype(-f), v4f));
static_assert(__is_same(decltype(+u), v4u));
static_assert(__is_same(decltype(g + g), g4i));
static_assert(__is_same(decltype(g * 2), g4i));

// Comparisons and logical not.
static_assert(__is_same(decltype(i < j), v4i));
static_assert(__is_same(decltype(u == u), v4i));
static_assert(__is_same(decltype(f != f), v4i));
static_assert(__is_same(decltype(d > d), v2l));
static_assert(__is_same(decltype(s <= s), v8s));
static_assert(__is_same(decltype(b == b), v4b));
static_assert(__is_same(decltype(!i), v4i));
static_assert(__is_same(decltype(!f), v4i));
static_assert(__is_same(decltype(g == g), g4i));
static_assert(__is_same(decltype(gd < gd), g2ll));
static_assert(__is_same(decltype(i == 0), v4i));

// Compound assignment and subscripts.
static_assert(__is_same(decltype(i[0]), int&));
static_assert(__is_same(decltype(f[1]), float&));
static_assert(__is_same(decltype(b[2]), bool&));
static_assert(__is_same(decltype(g[3]), int&));
static_assert(__is_same(decltype(v4i{}[0]), int&&));
static_assert(__is_same(decltype(v4i{1, 2, 3, 4}[1]), int&&));

void use(int k) {
  i[k] = 3;
  i[0] += 1;
  g[1] = k;
  f = 1.0f;
  i = k;
}
