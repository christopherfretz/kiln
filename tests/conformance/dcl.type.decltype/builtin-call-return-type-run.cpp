// decltype of a builtin call in a function template's signature: once the
// arguments are substituted the call is not dependent, so the
// specialization returns the call's type (a builtin first used inside a
// template is still a namespace-scope declaration). The return value was
// lowered as an unknown one-byte type, and the decltype mangled as a
// vendor placeholder instead of clang's `DTclL_Z <name> E ... E`.
//@ std: c++20
//@ expect: run
//@ stdout: 3 5 1 1
#include <cstddef>
#include <cstdio>
#include <new>
template <typename A> auto len(A a) -> decltype(__builtin_strlen(a)) {
  return __builtin_strlen(a);
}
template <typename A> auto hint(A a) -> decltype(__builtin_expect(a, 0)) {
  return a;
}
struct New {
  template <typename... A>
  auto operator()(A &&...a) const
      -> decltype(__builtin_operator_new(static_cast<A &&>(a)...)) {
    return __builtin_operator_new(static_cast<A &&>(a)...);
  }
};
static_assert(__is_same(decltype(len<const char *>), std::size_t(const char *)));
static_assert(__is_same(decltype(hint<long>), long(long)));
template std::size_t len(const char *);
// The specialization by its clang 19 name.
extern "C" std::size_t len_by_name(const char *) __asm__(
    "_Z3lenIPKcEDTclL_Z16__builtin_strlenEfp_EET_");
int main(int argc, char **) {
  void *p = New{}(std::size_t(argc + 15), std::align_val_t{64});
  bool aligned = p && reinterpret_cast<std::size_t>(p) % 64 == 0;
  ::operator delete(p, std::align_val_t{64});
  std::printf("%zu %ld %d %d\n", len("abc"), hint(5L), aligned,
              len_by_name("x") == 1);
}
