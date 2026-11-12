// [expr.const.const]/1, /3.1 (P4101R1, carrying P3603's constexpr pointers
// to immediate functions): a pointer to an immediate function is a
// consteval-only value, so an object holding one is an immediate object,
// which a `constexpr` variable or a template parameter object may be.
// `constexpr auto b = plus1;`, a `constexpr` array of them and a function
// pointer template argument (`C<plus1>`) are formed; a call through one
// (`b(1)`) is an immediate constant expression, fine in every manifestly
// constant-evaluated context, in the non-dependent spelling and in the
// dependent-then-instantiated ones (function template, class template
// member, lambda in a template, nested two levels). Code generation never
// emits or references `plus1` (kiln_codegen tests/runtime_reflection.rs).
// kiln only: GCC 16.1 rejects `constexpr auto b = plus1;` ("constant
// evaluation returns address of immediate function"; docs/status/deviations.md).
//@ expect: run
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P4101R1/P3603
//@ strict-diags
//@ stdout: 2 3 2
extern "C" int printf(const char*, ...);
consteval int plus1(int x) { return x + 1; }
template <auto V> struct C { static constexpr int v = V(1); };
constexpr auto b = plus1;
constexpr int (*arr[])(int) = {plus1, plus1};
struct H { int (*p)(int); int n; };
constexpr H h{plus1, 4};
static_assert(b(1) == 2 && arr[1](2) == 3 && h.p(h.n) == 5);
static_assert(C<plus1>::v == 2);
auto c = C<plus1>();
// function template
template <class T> constexpr int ft() { constexpr auto b = plus1; return b(T(1)); }
static_assert(ft<int>() == 2);
// class template member
template <class T> struct M { static constexpr auto b = plus1; static constexpr int r = b(2); };
static_assert(M<int>::r == 3);
// lambda in template
template <class T> constexpr int lt() { return [] { constexpr auto b = plus1; return b(T(3)); }(); }
static_assert(lt<int>() == 4);
// nested two levels
template <class T> struct O { template <class U> static constexpr int f() { constexpr auto b = plus1; return b(U(4)); } };
static_assert(O<int>::f<int>() == 5);
// dependent template argument
template <auto F> constexpr int call() { return C<F>::v; }
static_assert(call<plus1>() == 2);
template <class T> constexpr int via() { return C<plus1>::v + T(0); }
static_assert(via<int>() == 2);
// contexts
int ia[C<plus1>::v];
static_assert(sizeof(ia) == 2 * sizeof(int));
int arr2[b(3)];
template <int N> struct I {};
I<b(1)> i1;
template <class T> requires (b(1) == 2) struct R {};
R<int> r;
void sw(int x) { switch (x) { case b(0): break; default: break; } }
void ne() noexcept(b(1) == 2);
enum E { e = b(7) };
template <class T> constexpr int depctx() { int a[b(T(1))]; switch (T(1)) { case b(T(0)): break; } static_assert(b(T(1)) == 2); I<b(T(2))> v{}; (void)v; return sizeof(a) / sizeof(int); }
static_assert(depctx<int>() == 2);
// P4101R1 §4.4.2's example (GCC 16.1 accepts it too)
template <auto F> struct not_fn { constexpr bool operator()() const { return !F(); } };
consteval bool always_false() { return false; }
static_assert(not_fn<always_false>{}());
int main() {
  constexpr auto lb = plus1;
  int local[lb(2)];
  printf("%d %d %d\n", c.v, int(sizeof(local) / sizeof(int)), C<plus1>::v);
}
