// [temp.spec.partial.match]/2, [temp.deduct.type]/20, [temp.param]/6,
// [dcl.type.class.deduct]/2: the reflect-cpp shape of WP-1217. A qualified
// class template placeholder (`internal::sl _name`) beside a type parameter
// matches `Field<"name", T>` when the partial specialization is named through
// a pack expansion and an alias template; a non-pack placeholder parameter
// deduces from one element of a placeholder-typed pack (`Lit<"a">`), and a
// placeholder-typed pack deduces element by element. Before WP-1217 none of
// these partial specializations matched (a WP-1105 regression).
//@ std: c++20
//@ expect: run
//@ stdout: F 3 4 1
//@ stdout: N 2
//@ stdout: L 2 0
//@ stdout: P 3 0
using size_t = decltype(sizeof 0);
extern "C" int printf(const char *, ...);
namespace rfl {
namespace internal {
template <size_t N> struct sl {
  char arr_[N]{};
  constexpr sl(const char (&s)[N]) { for (size_t i = 0; i < N; ++i) arr_[i] = s[i]; }
};
} // namespace internal
template <internal::sl _name, class T> struct Field { T value; };
template <class... Fs> struct NamedTuple {};
template <internal::sl... fields_> class Lit {};
namespace internal {
template <class T> struct remove_ptr;
template <internal::sl _name, class T> struct remove_ptr<Field<_name, T>> {
  using FieldType = Field<_name, char[sizeof(T)]>;
};
template <class T> using remove_ptr_t = typename remove_ptr<T>::FieldType;
template <class T> struct remove_ptrs_nt;
template <class... Fs> struct remove_ptrs_nt<NamedTuple<Fs...>> {
  using type = NamedTuple<remove_ptr_t<Fs>...>;
};
} // namespace internal
namespace parsing::vaw {
template <class T> struct GetName { static constexpr int name_ = 0; };
template <internal::sl _name> struct GetName<Lit<_name>> {
  static constexpr internal::sl name_ = _name;
};
template <class T> struct Count { static constexpr int v = 0; };
template <internal::sl... fs> struct Count<Lit<fs...>> { static constexpr int v = sizeof...(fs); };
} // namespace parsing::vaw
} // namespace rfl

int main() {
  using NT = rfl::NamedTuple<rfl::Field<"ab", int *>, rfl::Field<"c", int>,
                             rfl::Field<"def", char>>;
  using R = rfl::internal::remove_ptrs_nt<NT>::type;
  // sizeof(int *) differs by target; print 3 for a pointer-sized array.
  printf("F %d %d %d\n",
         sizeof(rfl::internal::remove_ptr_t<rfl::Field<"ab", int *>>) == sizeof(int *) ? 3 : 0,
         (int)sizeof(rfl::internal::remove_ptr_t<rfl::Field<"c", int>>),
         (int)sizeof(rfl::internal::remove_ptr_t<rfl::Field<"def", char>>));
  (void)sizeof(R);
  printf("N %d\n", (int)sizeof(rfl::internal::remove_ptr_t<rfl::Field<"x", short>>));
  printf("L %d %d\n", (int)sizeof(rfl::parsing::vaw::GetName<rfl::Lit<"a">>::name_.arr_),
         rfl::parsing::vaw::GetName<int>::name_);
  printf("P %d %d\n", rfl::parsing::vaw::Count<rfl::Lit<"a", "bc", "d">>::v,
         rfl::parsing::vaw::Count<int>::v);
}
