// [expr.prim.req.nested]/1, [expr.prim.req.general]/5,
// [temp.constr.atomic]/3: a nested requirement whose constraint-expression
// names a local parameter of the requires-expression is substituted with
// that parameter substituted. When the substitution fails
// (`decltype(t.begin())` for a `T` with no `begin`), the requirement is
// not satisfied and the concept is false; checking its atoms one by one
// must not leave them naming the pattern's (dependent) parameter.
// Reduced from real code (a JSON library's range concept, checked
// through constrained partial specializations).
//@ expect: compile-ok
//@ std: c++20
template <class T, class U> constexpr bool same_v = false;
template <class T> constexpr bool same_v<T, T> = true;
template <class T, class U> concept same_as = same_v<T, U>;

template <class T>
concept range = requires(T& t) {
  requires !same_as<void, decltype(t.begin())>;
  requires !same_as<void, decltype(t.end())>;
};

struct R { int* begin(); int* end(); };
struct OnlyBegin { int* begin(); };
static_assert(range<R>);
static_assert(!range<int>);
static_assert(!range<OnlyBegin>);

// Two local parameters, the failing one second.
template <class T, class U>
concept addable = requires(T a, U b) {
  requires sizeof(a) > 0;
  requires same_as<decltype(a + b), decltype(b + a)>;
};
struct Opaque {};
static_assert(addable<int, long>);
static_assert(!addable<int, Opaque>);

// Through a class template's constrained partial specializations and a
// concept with a non-type parameter, as overloads select on it.
enum class Fmt : unsigned { json, bin };
template <Fmt F, class T> struct to;
template <class T> requires range<T> struct to<Fmt::bin, T> { static constexpr int v = 1; };
template <class T> requires (!range<T>) struct to<Fmt::bin, T> { static constexpr int v = 2; };
template <class T, Fmt F>
concept write_supported = requires { to<F, T>{}; };

template <auto& Opts, write_supported<Fmt::bin> T> constexpr int write(T&&) { return to<Fmt::bin, T>::v; }
template <write_supported<Fmt::bin> T> constexpr int write(T&&) { return 10 + to<Fmt::bin, T>::v; }
static_assert(write(R{}) == 11);
static_assert(write(0) == 12);

template <class T> constexpr int pick() { return 0; }
template <range T> constexpr int pick() { return 1; }
static_assert(pick<R>() == 1);
static_assert(pick<int>() == 0);
