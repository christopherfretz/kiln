// [dcl.spec.auto.general]/12: return type deduction in a templated
// function happens when the definition is instantiated, and returns in a
// discarded statement do not participate ([stmt.if]/2). This is the
// libc++ __get_comp_type shape (<compare>).
//@ expect: compile-ok
enum Cat : unsigned { None, Partial, Weak, Strong };
struct partial { static const partial equivalent; };
struct weak { static const weak equivalent; };
template <class T> constexpr Cat to_cat() { return Strong; }
template <> constexpr Cat to_cat<partial>() { return Partial; }
template <> constexpr Cat to_cat<void>() { return None; }

template <class... Ts, bool False = false>
constexpr auto get_comp_type() {
  constexpr Cat kinds[] = {Strong, to_cat<Ts>()...};
  constexpr Cat cat = kinds[sizeof...(Ts)];
  if constexpr (cat == None)
    return void();
  else if constexpr (cat == Partial)
    return partial::equivalent;
  else if constexpr (cat == Weak)
    return weak::equivalent;
  else
    static_assert(False, "unhandled case");
}

template <class... Ts> struct common { using type = decltype(get_comp_type<Ts...>()); };

template <class T> struct is_void { static constexpr bool value = false; };
template <> struct is_void<void> { static constexpr bool value = true; };
static_assert(is_void<common<void>::type>::value);
static_assert(!is_void<common<partial>::type>::value);

template <class T> auto pick(T t) {
  if constexpr (sizeof(T) == 1)
    return;
  else
    return t * 2.0;
}
static_assert(is_void<decltype(pick('a'))>::value);
static_assert(sizeof(decltype(pick(1))) == sizeof(double));

template <class T> decltype(auto) id(T &t) { return (t); }
int g;
static_assert(sizeof(id(g)) == sizeof(int));
