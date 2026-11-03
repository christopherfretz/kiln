// [expr.sub] [ACLE] `v[i]` on an SVE vector (clang
// `CreateBuiltinArraySubscriptExpr`, `isSveVLSBuiltinType`) is a vector
// element as for a GNU vector: the element type (`getSveEltType`) with the
// base's value category, a prvalue base materialized, the index an integer
// or unscoped enumeration, and no address of the element. `svbool_t`
// cannot be subscripted (checked before the index), nor can `__SVCount_t`
// (not an SVE vector), and the vector must be on the left. spec/sema.md
// "Vector subscript".
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve -ferror-limit=0
//@ strict-diags
//@ diag: 27:8 error "cannot assign to variable 'c' with const-qualified type 'const i32' (aka 'const __SVInt32_t')"
//@ diag: 26:18 note "variable 'c' declared const here"
//@ diag: 31:12 error "array subscript is not an integer"
//@ diag: 32:12 error "subscript of svbool_t is not allowed"
//@ diag: 33:12 error "subscript of svbool_t is not allowed"
//@ diag: 34:13 error "address of vector element requested"
//@ diag: 35:12 error "subscripted value is not an array, pointer, or vector"
//@ diag: 36:12 error "subscripted value is not an array, pointer, or vector"

typedef __SVInt32_t i32; typedef __SVBool_t b; typedef __SVCount_t c32;
typedef __SVFloat64_t f64;
enum E { A };
enum class F { B };

void f(const i32 c, i32 v, b p, c32 k, f64 d, float x) {
  c[0] = 1;
  int a = i32()[0] + v[A] + v[true] + v['a'];
  const int &r = v[a];
  double t = static_cast<f64 &&>(d)[1];
  int e = v[F::B];
  int g = p[x];
  int h = p[0];
  auto *q = &c[1];
  int i = k[0];
  int j = 0[v];
  (void)r; (void)t;
}

template <class T> int g(T v) { return v[1]; }
int h(i32 v) { return g(v); }
static_assert(__is_same(decltype(g<i32>), int(i32)));
