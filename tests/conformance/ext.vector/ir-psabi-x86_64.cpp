// GNU vector extension, x86-64 psABI §3.2.3: a vector wider than 128 bits is
// passed in memory without AVX, so clang's codegen warns at each call that
// passes or returns one (-Wpsabi, `checkFunctionCallABI`): `avx` up to 256
// bits, `avx512f` beyond. The first vector argument ends the check, else the
// return type is checked; the type printed is the callee's parameter, for a
// member call the one after it (clang counts `this`); through `...` the
// argument's own. Calls through a pointer are not checked (internal-linkage
// callees: ir-psabi-internal-x86_64.cpp). WP-187.
//@ expect: ir
//@ requires: clang>=22
//@ args: --target=x86_64-linux-gnu -ftrivial-auto-var-init=uninitialized
//@ strict-diags
//@ diag: 36:3 warning "AVX vector return of type 'g8i' (vector of 8 'int' values) without 'avx' enabled changes the ABI"
//@ diag: 37:3 warning "AVX vector argument of type 'g8d' (vector of 8 'double' values) without 'avx512f' enabled changes the ABI"
//@ diag: 38:3 warning "AVX vector argument of type 'g8i' (vector of 8 'int' values) without 'avx' enabled changes the ABI"
//@ diag: 39:3 warning "AVX vector argument of type 'v8i' (vector of 8 'int' values) without 'avx' enabled changes the ABI"
//@ diag: 41:3 warning "AVX vector argument of type 'g8i' (vector of 8 'int' values) without 'avx' enabled changes the ABI"
//@ diag: 42:6 warning "AVX vector argument of type 'B' (aka 'g8i') without 'avx' enabled changes the ABI"
//@ diag: 43:6 warning "AVX vector argument of type 'g8i' (vector of 8 'int' values) without 'avx' enabled changes the ABI"
//@ diag: 44:5 warning "AVX vector argument of type 'g8i' (vector of 8 'int' values) without 'avx' enabled changes the ABI"
//@ diag: 45:3 warning "AVX vector argument of type 'B' (aka 'g8i') without 'avx' enabled changes the ABI"
typedef int g8i __attribute__((vector_size(32)));
typedef double g8d __attribute__((vector_size(64)));
typedef int v8i __attribute__((ext_vector_type(8)));
typedef int g4i __attribute__((vector_size(16)));
using B = g8i;
g8i ret();
void take(g8d);
void take2(int, g8i, g8d);
v8i ext(v8i);
g4i small(g4i);
void var(int, ...);
void alias(int, B);
struct S { S(g8i); g8i m(g8i, B); g8i operator+(g8i); };
void h(S *s, g8i x, g8d y, v8i z, g4i w, void (*fp)(g8i)) {
  ret();
  take(y);
  take2(1, x, y);
  ext(z);
  small(w);
  var(1, x);
  s->m(x, x);
  *s + x;
  S a(x);
  alias(1, x);
  fp(x);
}
