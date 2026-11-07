// [temp.dep.type]/6, [temp.res.general]/4: in `using E = X::B::type;`
// inside `X<T> : B<T>`, `B` is not found in the current instantiation,
// which has a dependent base, so `X::B` is a member of an unknown
// specialization and `X::B::type` a dependent type in a type-only
// context, no `typename` needed. A namespace-scope alias template of the
// same name as the member (`env_t`) does not interfere (a sender
// library's `using __env2_t = __opstate::__opstate_base::__env2_t;`;
// WP-789).
//@ expect: run
//@ std: c++20
//@ stdout: 4 8 8
extern "C" int printf(const char *, ...);

namespace n {
template <class T> struct base {
  using env_t = T;
  using self = base;
};
template <class T> using env_t = T *;

template <class T> struct op final : base<env_t<T>> {
  using env_t = op::base::env_t;
  using also = op::self::env_t;
  env_t p = nullptr;
};

template <class T> struct plain : base<T> {
  using U = plain::self::env_t;
  U v{};
};
} // namespace n

int main() {
  n::op<int> o;
  n::plain<int> p;
  printf("%zu %zu %zu\n", sizeof(p.v), sizeof(o.p), sizeof(n::op<int>::also));
}
