// Mangling differential: a sweep of the Itanium `<expression>`
// productions in dependent signatures ([temp.over.link]/5; spec/abi.md
// "Mangling", WP-1029), in the style of clang's CodeGenCXX
// mangle-exprs.cpp: unary and binary operators, calls, casts, member
// access, sizeof/alignof/noexcept/typeid/throw, new, packs and folds,
// literals, braced lists and requires-expressions. clang 19 and 22
// agree on every name. Not here because clang cannot mangle them
// either: `__alignof__(expr)`, `sizeof...(fn-param-pack)`,
// `__builtin_offsetof`.
void *operator new(decltype(sizeof 0), void *) noexcept;
namespace std { class type_info; }
struct P { int m; int f(); static int s; int *ptr; P *operator->(); int operator*(); int operator<=>(const P &) const; P(); P(int); P(int, int); };
template <int N> struct S {};
template <class T> T val();
int g1(int);
template <class T> T tf(T);
template <class... T> int vf(T...);
// unary operators
template <class T> auto u1(T t) -> decltype(+t);
template <class T> auto u2(T t) -> decltype(-t);
template <class T> auto u3(T t) -> decltype(~t);
template <class T> auto u4(T t) -> decltype(!t);
template <class T> auto u5(T t) -> decltype(*t);
template <class T> auto u6(T t) -> decltype(&t);
template <class T> auto u7(T t) -> decltype(++t);
template <class T> auto u8(T t) -> decltype(t++);
template <class T> auto u9(T t) -> decltype(--t);
template <class T> auto u10(T t) -> decltype(t--);
// binary operators
template <class T> auto b1(T t) -> decltype(t - t, t * t, t / t, t % t);
template <class T> auto b2(T t) -> decltype(t & t, t | t, t ^ t, t << 1, t >> 1);
template <class T> auto b3(T t) -> decltype(t == t, t != t, t < t, t > t, t <= t, t >= t);
template <class T> auto b4(T t) -> decltype(t && t, t || t);
template <class T> auto b5(T t) -> decltype(t = t, t -= 1, t *= 1, t /= 1, t %= 1);
template <class T> auto b6(T t) -> decltype(t |= 1, t ^= 1, t >>= 1);
template <class T> auto b7(T t) -> decltype(t <=> t);
template <class T> auto b8(T t) -> decltype(t ? t : t);
template <class T> auto b9(T t) -> decltype(t[0]);
template <class T> auto b10(T t) -> decltype(t.*(&P::m));
template <class T> auto b11(T t) -> decltype(t->*(&P::m));
// calls, conversions
template <class T> auto c1(T t) -> decltype(g1(t));
template <class T> auto c2(T t) -> decltype(tf(t));
template <class T> auto c3(T t) -> decltype((g1)(t));
template <class T> auto c4(T t) -> decltype(t.f());
template <class T> auto c5(T t) -> decltype(t->f());
template <class T> auto c6(T t) -> decltype(T(t));
template <class T> auto c7(T t) -> decltype(T());
template <class T> auto c9(T t) -> decltype(T{t});
template <class T> auto c10(T t) -> decltype(T{});
template <class T> auto c11(T t) -> decltype((long)t);
template <class T> auto c12(T t) -> decltype(static_cast<long>(t));
template <class T> auto c13(T t) -> decltype(const_cast<T&>(t));
template <class T> auto c14(T t) -> decltype(reinterpret_cast<long>(t));
template <class T> auto c15(T t) -> decltype(dynamic_cast<P*>(t));
template <class T> auto c16(T t) -> decltype(val<T>());
template <class T> auto c17(T t) -> decltype(t.T::f());
template <class T> auto c18(T t) -> decltype(t.m);
template <class T> auto c19(T t) -> decltype(t->m);
template <class T> auto c20(T t) -> decltype(T::s);
template <class T> auto c21(T t) -> decltype(::g1(t));
template <class T> auto c22(T t) -> decltype(t.operator*());
template <class T> auto c23(T t) -> decltype(t.~T());
// sizeof / alignof / noexcept / typeid (throw, `tw`/`tr`, is in
// tests/conformance/temp.over.link: this differential runs -fno-exceptions)
template <class T> auto s1(T t) -> S<sizeof(T)>;
template <class T> auto s2(T t) -> S<sizeof t>;
template <class T> auto s3(T t) -> S<alignof(T)>;
template <class T> auto s5(T t) -> S<noexcept(t + 1)>;
template <class T> auto s6(T t) -> decltype(typeid(T));
template <class T> auto s7(T t) -> decltype(typeid(t));
template <class... T> auto s10(T... t) -> S<sizeof...(T)>;
// new / delete
template <class T> auto n1(T t) -> decltype(new T);
template <class T> auto n2(T t) -> decltype(new T(t));
template <class T> auto n3(T t) -> decltype(new T{t});
template <class T> auto n4(T t) -> decltype(new T[2]);
template <class T> auto n5(T t) -> decltype(::new T());
template <class T> auto n6(T t) -> decltype(new (&t) T(t));
template <class T> auto n7(T t) -> decltype(new T[t]{1, 2});
// packs and folds
template <class... T> auto p1(T... t) -> decltype(vf(t...));
template <class... T> auto p2(T... t) -> decltype((t + ...));
template <class... T> auto p3(T... t) -> decltype((... + t));
template <class... T> auto p4(T... t) -> decltype((t + ... + 0));
template <class... T> auto p5(T... t) -> decltype((0 + ... + t));
template <class... T> auto p6(T... t) -> decltype((t , ...));
template <class... T> auto p7(T... t) -> decltype(vf(t + 1 ...));
template <class... T> auto p8(T... t) -> decltype(vf(T(t)...));
// literals
template <class T> auto l1(T t) -> decltype(t + 'a');
template <class T> auto l2(T t) -> decltype(t + true);
template <class T> auto l3(T t) -> decltype(t == nullptr);
template <class T> auto l4(T t) -> decltype(t + 1.5);
template <class T> auto l5(T t) -> decltype(t + 1.5f);
template <class T> auto l6(T t) -> decltype(t + "s");
template <class T> auto l7(T t) -> decltype(t + u8'a');
template <class T> auto l8(T t) -> decltype(t + L'a');
template <class T> auto l9(T t) -> decltype(t + -1);
template <class T> auto l10(T t) -> decltype(t + 1u + 1l + 1ul + 1ll + 1ull);
template <class T> auto l11(T t) -> decltype(t + __null);
// braced init lists / misc
template <class T> auto i1(T t) -> decltype(tf<T>({t}));
template <class T> auto i2(T t) -> decltype(g1({t}));
template <class T> auto r1(T t) -> S<requires { t + 1; }>;
template <class T> auto r2(T t) -> S<requires(T u) { u.m; typename T::type; }>;
template <class T> auto x2(T t) -> decltype(t, 0);
template <class T> auto x3(T t) -> decltype(&T::m);
template <class T> auto x4(T t) -> decltype(&T::f);
template <class T> auto x5(T t) -> decltype(__builtin_addressof(t));
template <class T> auto x10(T t) -> decltype(__builtin_expect(t, 1));
template <class T> auto x11(T t) -> decltype(sizeof(T::m));
template <class T> auto x12(T t) -> decltype(T::m);
void use() {
  int i = 0; P p; P *pp = &p; int *ip = &i;
  u1(i); u2(i); u3(i); u4(i); u5(ip); u6(i); u7(i); u8(i); u9(i); u10(i);
  b1(i); b2(i); b3(i); b4(i); b5(i); b6(i); b7(p); b8(i); b9(ip); b10(p); b11(pp);
  c1(i); c2(i); c3(i); c4(p); c5(pp); c6(i); c7(i); c11(i); c12(i); c13(i); c14(ip); c15(pp); c16(i);
  c17(p); c18(p); c19(pp); c20(p); c21(i); c22(p); c9(i); c10(i); c23(p);
  s1(i); s2(i); s3(i); s5(i); s6(i); s7(i); s10(1, 2);
  n1(i); n2(i); n3(i); n4(i); n5(i); n6(i); n7(i);
  p1(1, 2); p2(1, 2); p3(1, 2); p4(1, 2); p5(1, 2); p6(1, 2); p7(1, 2); p8(1, 2);
  l1(i); l2(i); l3(ip); l4(i); l5(i); l6(0); l7(i); l8(i); l9(i); l10(i); l11(ip);
  i1(i); i2(i); r1(i); r2(p); x2(i); x3(p); x4(p); x5(i); x10(1L); x11(p); x12(p);
}
