// The GNU element-wise conditional `c ? a : b` with a vector condition
// (clang CXXCheckConditionalOperands + CheckVectorConditionalTypes): the
// condition is not converted to bool; two vector arms keep their type,
// a vector and a scalar arm follow CheckVectorOperands, two scalar arms
// give a vector (GNU or ext, like the condition) of their common type
// with the condition's lane count. A scalar condition with a vector arm
// also goes through CheckVectorOperands. Result types are compared with
// typedefs: they are canonically equal to the unnamed vector types (WP-173).
typedef int g4i __attribute__((vector_size(16)));
typedef unsigned g4u __attribute__((vector_size(16)));
typedef float g4f __attribute__((vector_size(16)));
typedef long long g2ll __attribute__((vector_size(16)));
typedef double g2d __attribute__((vector_size(16)));
typedef short g8s __attribute__((vector_size(16)));
typedef int v4i __attribute__((ext_vector_type(4)));
typedef float v4f __attribute__((ext_vector_type(4)));
typedef char v4c __attribute__((ext_vector_type(4)));
typedef bool v4b __attribute__((ext_vector_type(4)));

template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

g4i c, a;
g4u u;
g4f f;
g2ll c2;
v4i ec, ea;
v4f ef;
v4b eb;
v4c ech;
int i;
float x;
double d;

static_assert(same<decltype(c ? a : a), g4i>);
static_assert(same<decltype(c ? f : f), g4f>);
static_assert(same<decltype(u ? u : 1u), g4u>);
static_assert(same<decltype(c ? a : 1), g4i>);
static_assert(same<decltype(c ? 2 : f), g4f>);
static_assert(same<decltype(c ? i : i), g4i>);
static_assert(same<decltype(c ? x : 1), g4f>);
static_assert(same<decltype(c2 ? d : 1), g2d>);
static_assert(same<decltype(ec ? ea : ea), v4i>);
static_assert(same<decltype(ec ? ef : 1.5f), v4f>);
static_assert(same<decltype(ec ? i : 0), v4i>);
static_assert(same<decltype(ech ? 'a' : 'b'), v4c>);
static_assert(same<decltype(eb ? eb : eb), v4b>);
// A scalar condition: CheckVectorOperands on the arms.
static_assert(same<decltype(i ? a : a), g4i &>);
static_assert(same<decltype(i ? a : 1), g4i>);
static_assert(same<decltype(i ? 1.5f : ef), v4f>);
static_assert(same<decltype(i ? a : u), g4i>);
static_assert(same<decltype(i ? a : f), g4i>);
static_assert(same<decltype(i ? ea : a), v4i>);
// The result is a prvalue.
static_assert(same<decltype((c ? a : a)), g4i>);

template <class C, class T> auto sel(C k, T p, T q) { return k ? p : q; }
static_assert(same<decltype(sel(c, a, a)), g4i>);
static_assert(same<decltype(sel(c, i, i)), g4i>);
static_assert(same<decltype(sel(i, a, a)), g4i>);
template <class T> auto splat(T k) { return k ? 1LL : 2; }
static_assert(same<decltype(splat(c2)), g2ll>);

g4i use(g4i k, g4i p, g4i q) { return k ? p : q; }
