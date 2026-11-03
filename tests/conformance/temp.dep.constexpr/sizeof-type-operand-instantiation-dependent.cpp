// [temp.dep.constexpr]/2, [temp.dep.type]/9, [temp.deduct.general]/8:
// `sizeof(first_t<int, decltype(T::x)>)` is not value-dependent (its operand
// is the alias `int`), but the operand type is instantiation-dependent. As a
// template argument it is kept as written and substituted again at each
// deduction, so a failure in `decltype(T::x)` removes the candidate; it is
// not folded away when the template is defined. Its canonical argument is
// the value, so `enable_if<sizeof(...) == 4>` names `enable_if<true>`.
// The same holds for `alignof`, a type trait's operands and a qualifier.
//@ expect: compile-ok
//@ strict-diags
template <class...> using void_t = void;
template <class U, class...> using first_t = U;
template <class T, class U> struct is_same { static constexpr bool value = false; };
template <class T> struct is_same<T, T> { static constexpr bool value = true; };
template <bool, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <bool B, class T = void> using enable_if_t = typename enable_if<B, T>::type;
struct A { int x; };
struct Bn {};

template <class T, enable_if_t<sizeof(void_t<decltype(T::x)>*) != 0, int> = 0> constexpr int f(T) { return 1; }
template <class T> constexpr int f(...) { return 2; }
static_assert(f<Bn>(Bn{}) == 2);
static_assert(f<A>(A{}) == 1);

template <class T, enable_if_t<sizeof(first_t<int, decltype(T::x)>) == sizeof(int), int> = 0> constexpr int g(T) { return 1; }
template <class T> constexpr int g(...) { return 2; }
static_assert(g<Bn>(Bn{}) == 2);
static_assert(g<A>(A{}) == 1);

template <class T, enable_if_t<alignof(first_t<int, decltype(T::x)>) != 0, int> = 0> constexpr int h(T) { return 1; }
template <class T> constexpr int h(...) { return 2; }
static_assert(h<Bn>(Bn{}) == 2);
static_assert(h<A>(A{}) == 1);

template <class T, enable_if_t<__is_same(first_t<int, decltype(T::x)>, int), int> = 0> constexpr int t(T) { return 1; }
template <class T> constexpr int t(...) { return 2; }
static_assert(t<Bn>(Bn{}) == 2);
static_assert(t<A>(A{}) == 1);

// A class template-id names the specialization of the folded value, and a
// partial specialization re-substitutes the written argument.
template <class T, class = void> struct has { static constexpr int v = 0; };
template <class T> struct has<T, typename enable_if<sizeof(first_t<int, decltype(T::x)>) == 4>::type> { static constexpr int v = 1; };
static_assert(has<A>::v == 1);
static_assert(has<Bn>::v == 0);
template <class T, class = void> struct has2 { static constexpr int v = 0; };
template <class T> struct has2<T, typename enable_if<is_same<void, void_t<decltype(T::x)>>::value>::type> { static constexpr int v = 1; };
static_assert(has2<A>::v == 1);
static_assert(has2<Bn>::v == 0);
