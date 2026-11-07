// [temp.dep.type]/1, [temp.spec.partial.general]/7, [temp.class.general]/3:
// in the declarator qualifier `F<T, typename X<T>::type>::`, only `F<...>`
// is the scope being entered; `X<T>` in its argument list is an ordinary
// dependent specialization, not the current instantiation of `X`, so the
// argument stays the dependent `typename X<T>::type` and the qualifier
// names the partial specialization (fmt 10 format.h
// `formatter<T, Char, enable_if_t<type_constant<T, Char>::value !=
// custom_type>>::format`, reached from folly; WP-726).
//@ std: c++20
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 3
extern "C" int printf(const char*, ...);
template <bool B, class T = void> struct en {};
template <class T> struct en<true, T> { using type = T; };
template <bool B, class T = void> using en_t = typename en<B, T>::type;
template <class T> struct tc { static constexpr int value = 1; };
template <class T> struct X { using type = void; };

template <class T, class E = void> struct F;
template <class T> struct F<T, typename X<T>::type> {
  int g() const;
  template <class C> int f(C) const;
};
template <class T> int F<T, typename X<T>::type>::g() const { return 1; }
template <class T>
template <class C>
int F<T, typename X<T>::type>::f(C c) const { return c; }

template <class T, class E = void> struct G;
template <class T> struct G<T, en_t<tc<T>::value != 0>> {
  template <class C> auto h(C c) const -> int;
};
template <class T>
template <class C>
auto G<T, en_t<tc<T>::value != 0>>::h(C c) const -> int { return c + 1; }

int main() { printf("%d %d %d\n", F<int>().g(), F<long>().f(2), G<int>().h(2)); }
