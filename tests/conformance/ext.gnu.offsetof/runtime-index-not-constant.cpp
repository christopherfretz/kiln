// __builtin_offsetof with an index that is not a constant is not a
// constant expression ([expr.const]): rejected where one is required,
// with the evaluation's note at the index; a constexpr function whose
// index is its parameter is fine until a call needs it. The
// non-standard-layout warning is still given ([support.types.layout]/1).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:15 error "static assertion expression is not an integral constant expression"
//@ diag: 25:39 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 24:5 note "declared here"
//@ diag: 26:18 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 26:49 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 24:5 note "declared here"
//@ diag: 28:3 error "non-type template argument is not a constant expression"
//@ diag: 28:27 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 24:5 note "declared here"
//@ diag: 33:8 error "case value is not a constant expression"
//@ diag: 33:32 note "function parameter 'i' with unknown value cannot be used in a constant expression"
//@ diag: 31:12 note "declared here"
//@ diag: 36:9 warning "on non-standard-layout type 'NS'"
typedef __SIZE_TYPE__ size_t;
struct S { int n; char a[8][4]; };
struct NS { virtual void f(); int a[4]; };
int k;
static_assert(__builtin_offsetof(S, a[k][0]) == 4);
constexpr size_t c = __builtin_offsetof(S, a[1][k]);
template <size_t N> struct T {};
T<__builtin_offsetof(S, a[k])> t;
constexpr size_t f(int i) { return __builtin_offsetof(S, a[i][i]); }
static_assert(f(1) == 9);
void g(int i) {
  switch (i) {
  case __builtin_offsetof(S, a[i]):
    break;
  }
  (void)__builtin_offsetof(NS, a[i]);
}
