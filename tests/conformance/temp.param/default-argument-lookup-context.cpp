// [temp.param]/12, [temp.dep.candidate]/1, [temp.res.general]/1: the names
// in a default template argument are looked up from the template's scope,
// not from the scope of the use that needs the default. An unqualified
// call with explicit template arguments (`declval<Ts>()`, in a pack
// expansion or not) in the default of a template declared in a
// namespace, a nested namespace or a class resolves there when the
// template is used from outside (it was "use of undeclared identifier
// 'declval'" before WP-378), and a function visible only at the use is
// not found (the default is a substitution failure, so the other
// overload is chosen).
//@ expect: compile-ok
//@ strict-diags
namespace n {
template <class T> T &&declval();
template <class T, class = decltype(declval<T>())> int one() { return 1; }
template <class... Ts, class = decltype((declval<Ts>(), ...))> int fold() {
  return sizeof...(Ts);
}
template <class... Ts, class = void (*)(decltype(declval<Ts>())...)>
int list() { return sizeof...(Ts); }
template <class T, int = sizeof(declval<T>())> int nttp() { return 4; }
template <class T, class = decltype(declval<T>())> struct S {
  static constexpr int value = sizeof(T);
};

namespace in {
// Found by ordinary lookup in the enclosing namespace.
template <class... Ts, class = decltype((declval<Ts>(), ...))> int nested() {
  return sizeof...(Ts);
}
} // namespace in

template <class... Ts> struct C {
  template <class U> static U &&make();
  template <class... Us, class = decltype((make<Us>(), ...))>
  static int member() { return sizeof...(Us); }
  template <class... Us, class = decltype((declval<Us>(), ...))>
  static int outer() { return sizeof...(Us); }
};

struct Plain {
  template <class U> static U &&make();
  template <class... Us, class = decltype((make<Us>(), ...))>
  static int member() { return sizeof...(Us); }
};

template <class T, class = decltype(helper<T>(T()))> constexpr int pick(int) {
  return 1;
}
template <class T> constexpr int pick(long) { return 2; }
} // namespace n

// Visible only here: the default above must not find it.
template <class T> int helper(T) { return 0; }

int use() {
  return n::one<int>() + n::fold<int, long>() + n::list<int, char>() +
         n::nttp<int>() + n::S<int>::value + n::in::nested<int>() +
         n::C<int>::member<int, char>() + n::C<int>::outer<char>() +
         n::Plain::member<int>();
}

static_assert(sizeof(n::fold<int, long>()) == sizeof(int));
static_assert(n::S<char>::value == 1);

static_assert(n::pick<int>(0) == 2); // the second overload
