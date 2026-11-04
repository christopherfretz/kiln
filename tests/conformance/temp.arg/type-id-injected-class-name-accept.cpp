// [temp.arg.general]/2, [dcl.ambig.res]/3, [temp.local]/1: in a
// template-argument, `void(CT)` is a type-id (a function type) when `CT`
// names a type, and a class template's injected-class-name names the
// class inside its own definition unless followed by `<`. The same holds
// in a parameter-declaration-clause, in `sizeof`, `decltype` operands and
// in an alias-declaration. The other type names a tentative parse sees
// after `T(` are checked alongside.
//@ expect: compile-ok
template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <class T> struct decay { using type = T; };
template <class T> struct decay<T &&> { using type = T; };

namespace ns {
struct C { int m; int f(); };
template <class T> struct V {};
} // namespace ns
using ns::C;

template <class... Ts> struct CT {
  // The shape that motivated the fix: a constructor template constrained
  // on the argument not being the class itself.
  template <class First,
            typename enable_if<!same<void(CT), void(typename decay<First>::type)>::value,
                               bool>::type = true>
  explicit CT(First &&) {}
  CT() = default;

  static_assert(same<void(CT), void(CT<Ts...>)>::value);
  static_assert(same<void(CT<Ts...>), void(CT<Ts...>)>::value);
  static_assert(same<int(CT, CT), int(CT<Ts...>, CT<Ts...>)>::value);
  static_assert(same<CT(CT), CT<Ts...>(CT<Ts...>)>::value);
  static_assert(same<void(const CT &), void(const CT<Ts...> &)>::value);
  static_assert(same<void(::ns::C), void(C)>::value);
  static_assert(same<void(ns::V<int>), void(ns::V<int>)>::value);
  static_assert(same<int (C::*)(), decltype(&C::f)>::value);
  static_assert(same<int(C::*), int C::*>::value);
  static_assert(sizeof(void (*)(CT)) == sizeof(void (*)()));
  static_assert(same<decltype(static_cast<void (*)(CT)>(nullptr)), void (*)(CT<Ts...>)>::value);

  using fn = void(CT);
  static_assert(same<fn, void(CT<Ts...>)>::value);

  // A parameter of function type `void(CT)`, adjusted to a pointer.
  static void g(void(CT));
  static_assert(same<decltype(g), void(void (*)(CT<Ts...>))>::value);

  void body() { static_assert(same<void(CT), void(CT<Ts...>)>::value); }
};

template <class T> struct U {
  using type = int;
  static_assert(same<void(T), void(T)>::value);
  static_assert(same<void(typename T::type), void(int)>::value);
  static_assert(same<void(U), void(U<T>)>::value);
};

struct H { using type = int; };

CT<int> a;
CT<int> b(3);
CT<int, char> c;
U<H> u;
