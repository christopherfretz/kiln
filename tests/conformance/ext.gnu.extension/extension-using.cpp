// GNU `__extension__` before an alias-declaration or using-declaration, at
// namespace, class and block scope (libc++ <random>
// `__extension__ using __uint128_t = unsigned __int128;`). Verified against
// clang 19.
//@ expect: compile-ok
__extension__ using u128 = unsigned __int128;
namespace N { int v; }
__extension__ using N::v;
struct S {
  __extension__ using T = __int128;
  T t;
};
int f() {
  __extension__ __extension__ using U = long;
  U u = v;
  return int(u);
}
static_assert(sizeof(u128) == 16 && sizeof(S::T) == 16);
