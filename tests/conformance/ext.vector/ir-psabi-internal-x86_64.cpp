// GNU vector extension, x86-64 psABI §3.2.3: -Wpsabi (ir-psabi-x86_64.cpp)
// for internal-linkage callees ([basic.link]/4): a static function, one in
// an unnamed namespace, a lambda's and a local class's call operator. clang
// 19 warns at each call; clang 20+ skips a callee that is not externally
// visible, as its ABI is the compiler's to choose. WP-187.
//@ expect: ir
//@ requires: clang<22
//@ args: --target=x86_64-linux-gnu
//@ strict-diags
//@ diag: 24:3 warning "AVX vector argument of type 'g8i' (vector of 8 'int' values) without 'avx' enabled changes the ABI"
//@ diag: 25:3 warning "AVX vector return of type 'g8i' (vector of 8 'int' values) without 'avx' enabled changes the ABI"
//@ diag: 26:3 warning "AVX vector argument of type 'g8i' (vector of 8 'int' values) without 'avx' enabled changes the ABI"
//@ diag: 27:3 warning "AVX vector argument of type 'g8i' (vector of 8 'int' values) without 'avx' enabled changes the ABI"
//@ diag: 29:3 warning "AVX vector argument of type 'g8i' (vector of 8 'int' values) without 'avx' enabled changes the ABI"
//@ diag: 30:3 warning "AVX vector argument of type 'g8i' (vector of 8 'int' values) without 'avx' enabled changes the ABI"
//@ diag: 22:29 warning "AVX vector argument of type 'g8i' (vector of 8 'int' values) without 'avx' enabled changes the ABI"
typedef int g8i __attribute__((vector_size(32)));
static void sf(g8i) {}
static g8i sr() { return g8i{}; }
namespace { void an(g8i) {} }
void ex(g8i);
static void caller(g8i x) { ex(x); }
void h(g8i x) {
  sf(x);
  sr();
  an(x);
  [](g8i) {}(x);
  struct L { void operator()(g8i) const {} } l;
  l(x);
  caller(x);
}
