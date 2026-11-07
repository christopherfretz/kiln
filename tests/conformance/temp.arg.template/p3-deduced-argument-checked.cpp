// [temp.arg.template]/3, [temp.deduct.type]/8: a template template argument
// deduced from a template-id must match its template template parameter,
// else deduction fails. `T<Ts...>` for `template<class...> class T` does not
// match `iseq<int>` (whose second parameter is a non-type pack), so the
// partial specialization for `iseq<T, Is...>` is the only match (not an
// ambiguity) and the function template `f` is not viable for it.
//@ expect: run
//@ std: c++20
//@ strict-diags
template <class T, T...> struct iseq {};
template <class...> struct list {};

template <class X> struct apply { static constexpr int k = 0; };
template <template <class...> class T, class... Ts>
struct apply<T<Ts...>> { static constexpr int k = 1; };
template <class T, T... Is>
struct apply<iseq<T, Is...>> { static constexpr int k = 2; };
static_assert(apply<list<int, char>>::k == 1);
static_assert(apply<list<>>::k == 1);
static_assert(apply<iseq<int>>::k == 2);
static_assert(apply<iseq<int, 1, 2>>::k == 2);

template <template <class...> class T, class... Ts> char f(T<Ts...>);
long f(...);
static_assert(sizeof(f(list<int>{})) == sizeof(char));
static_assert(sizeof(f(iseq<int>{})) == sizeof(long));

int main() { return apply<iseq<unsigned long>>::k - 2; }
