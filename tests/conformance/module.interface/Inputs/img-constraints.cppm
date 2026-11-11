export module imgc;
export namespace ic {
template <class T> concept Small = sizeof(T) <= 2;
template <class T> concept Big = !Small<T> && sizeof(T) >= 4;
template <class T, class U> concept Same = __is_same(T, U);
template <Small T> int pick(T) { return 1; }
template <class T> requires Big<T> int pick(T) { return 2; }
template <class T> int pick2(T) requires Same<T, long> { return 3; }
template <class T> int pick2(T) { return 4; }
template <Small... Ts> int count(Ts...) { return sizeof...(Ts); }
template <class T> struct Box {
  int h() requires Small<T> { return 5; }
  int h() requires Big<T> { return 6; }
};
template <Big T> using BigAlias = T;
template <Small T> constexpr int vt = sizeof(T);
template <class T> concept Fooable = requires(T t) {
  t.foo();
  typename T::type;
  { t.foo() } -> Same<int>;
  { t.bar() } noexcept;
  requires sizeof(T) > 1;
};
template <class T> concept Addable = requires(T a, T b) { a + b; };
template <class... T> concept Packy = requires(T... ts) { (ts + ...); };
template <class T> int f(T t) requires requires { t.foo(); } { return t.foo(); }
template <class T> int g(T) { return 0; }
template <Fooable T> int g(T t) { return t.foo() + 10; }
template <class T> constexpr bool has_foo = requires(T t) { t.foo(); };
}
