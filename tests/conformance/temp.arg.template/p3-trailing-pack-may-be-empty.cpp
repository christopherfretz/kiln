// [temp.arg.template]/3-4: `template<class, class...> class S` takes a
// template whose only parameter is `class T`: `S`'s trailing pack, written
// as an argument for `T`'s list, may expand to nothing. The rebinding
// partial specialization (libstdc++ `__replace_first_arg`) matches an
// allocator-like `one<int>`, and `Y<one>` is valid.
//@ expect: run
//@ std: c++20
//@ strict-diags
template <class T> struct one {};
template <class T, class U> struct two {};

template <class X, class U> struct rebind {};
template <template <class, class...> class S, class U, class T, class... Ts>
struct rebind<S<T, Ts...>, U> { using type = S<U, Ts...>; };
static_assert(__is_same(rebind<one<int>, long>::type, one<long>));
static_assert(__is_same(rebind<two<int, char>, long>::type, two<long, char>));

template <template <class, class...> class S> struct Y { static constexpr int k = 1; };
static_assert(Y<one>::k == 1 && Y<two>::k == 1);

int main() { return Y<one>::k - 1; }
