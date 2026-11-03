// GNU `__extension__` as the first token inside parentheses (WP-525;
// libstdc++ 16 <bits/stl_construct.h> `(__extension__ expr)`): it is
// the unary extension operator, so `(__extension__ e)` is a parenthesized
// expression, not a cast -- the scan only takes `(` for a type-id when a
// decl-specifier follows the keyword (clang `ParseCastExpression`
// `kw___extension__`; `isCXXDeclarationSpecifier`). Verified against
// clang 19.
//@ expect: compile-ok
//@ args: -Wno-vla-cxx-extension
//@ strict-diags
int* f(void* p) {
  auto s = (__extension__ 1 + 2);
  auto t = (__extension__ reinterpret_cast<int*>(p));
  auto u = (__extension__ __extension__ sizeof(int));
  return s + u > 0 ? t : nullptr;
}
template <class T> constexpr T twice(T v) { return (__extension__ v * 2); }
static_assert(twice(21) == 42);
static_assert((__extension__ 1 + 2) == 3);
void g() {
  __extension__ int x = 1;
  __extension__ long long y = (__extension__ x);
  (void)y;
}
template <class T> T* as_array(void* p, unsigned long n) {
  auto r = (__extension__ reinterpret_cast<T(*)[n]>(p));
  return *r;
}
int* h(void* p) { return as_array<int>(p, 4); }
