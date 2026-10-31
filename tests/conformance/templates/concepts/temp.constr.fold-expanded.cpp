//@ expect: compile-ok
// [temp.constr.fold] (P2963, Q17): fold-expanded constraints over a
// pack: `(C<Ts> && ...)` / `(C<Ts> || ...)` in requires-clauses, nested
// requirements, partial specializations and concept definitions; an
// empty pack satisfies `&&`.
template <class T> concept Int = __is_same(T, int);
template <class T> concept Small = sizeof(T) <= 4;
template <class... Ts> requires (Int<Ts> && ...) constexpr int f(Ts...) { return 1; }
template <class... Ts> requires (Int<Ts> || ...) constexpr int g(Ts...) { return 2; }
static_assert(f(1, 2, 3) == 1);
static_assert(f() == 1);
static_assert(g(1L, 2) == 2);
template <class... Ts> constexpr bool all_int = requires { requires (Int<Ts> && ...); };
static_assert(all_int<int, int>);
static_assert(!all_int<int, long>);
template <class... Ts> struct S { static constexpr int v = 0; };
template <class... Ts> requires (Small<Ts> && ...) struct S<Ts...> { static constexpr int v = 1; };
static_assert(S<int, char>::v == 1);
static_assert(S<int, double>::v == 0);
template <class... Ts> concept AllInt = (Int<Ts> && ...);
static_assert(AllInt<int, int>);
static_assert(!AllInt<int, char>);
