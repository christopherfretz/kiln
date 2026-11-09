// [over.match.best.general]/2.5-2.8 for reversed rewritten candidates
// (WP-1111). Between two templates, constraints are compared only when
// the function parameters correspond positionally *as declared*: clang
// and GCC do not apply [temp.func.order]/6.2.1 (reversing the reversed
// candidate's parameters), so a constrained `operator<=>(L, S<X>)` used
// reversed for `s < l` is not more specialized than an unconstrained
// `operator<(S<X>, L)`, which wins by /2.8 (t2). Between two
// non-templates, /2.6 compares the parameter-type-lists after reversal
// (clang `sameFunctionParameterTypeLists`), so the more constrained
// reversed candidate wins before /2.8 (nt). Each operator prints its name.
//@ expect: run
//@ stdout: t1: <=>|t1: <=>|t2: <|t2: >=|t3: <=>|nt: <=>|nt: <=>|eq: W,T|eq: W,T|eq: W,T|vt: V,U|vt: U,V|
#include <compare>
extern "C" int printf(const char*, ...);
#define SAY(s) printf("%s|", s)
template <class T, class U> concept Cmp = requires(T t, U u) { t <=> u; };
template <class T> concept C = sizeof(T) > 0;
template <class T> concept D = C<T> && alignof(T) > 0;
struct Dur {
  long v;
  friend constexpr auto operator<=>(Dur, Dur) = default;
};
template <class X> struct ST { Dur d; };
namespace t1 { // not reversed: the more constrained `<=>` template wins (/2.5)
struct L {
  template <class X> friend bool operator<(const L&, const ST<X>&) { SAY("t1: <"); return true; }
  template <Cmp<Dur> X> friend std::strong_ordering operator<=>(const L&, const ST<X>&) { SAY("t1: <=>"); return std::strong_ordering::less; }
};
}
namespace t2 { // reversed: constraints not compared; the non-rewritten `<`/`>=` (/2.8)
struct L {
  template <class X> friend bool operator<(const ST<X>&, const L&) { SAY("t2: <"); return true; }
  template <class X> friend bool operator>=(const ST<X>&, const L&) { SAY("t2: >="); return true; }
  template <Cmp<Dur> X> friend std::strong_ordering operator<=>(const L&, const ST<X>&) { SAY("t2: <=>"); return std::strong_ordering::less; }
};
}
namespace t3 { // reversed, parameters the same as declared: constraints compared
struct TA {};
struct TB {};
template <class T> concept A = __is_same(T, TA) || __is_same(T, TB);
template <class T> concept IsTB = __is_same(T, TB);
template <class X, class Y> requires A<X> bool operator<(const X&, const Y&) { SAY("t3: <"); return true; }
template <class X, class Y> requires A<X> && IsTB<X>
std::strong_ordering operator<=>(const X&, const Y&) { SAY("t3: <=>"); return std::strong_ordering::less; }
}
namespace nt { // non-templates: /2.6 after reversal
struct U {};
template <class T> struct W {
  friend bool operator<(const T&, const W&) requires C<T> { SAY("nt: <"); return true; }
  friend bool operator<(const W&, const T&) requires C<T> { SAY("nt: <"); return true; }
  friend std::strong_ordering operator<=>(const W&, const T&) requires D<T> { SAY("nt: <=>"); return std::strong_ordering::less; }
  friend bool operator==(const T&, const W&) requires C<T> { SAY("eq: T,W"); return true; }
  friend bool operator==(const W&, const T&) requires D<T> { SAY("eq: W,T"); return true; }
};
}
namespace vt { // templates and reversed `==`: as t2 and t1
struct U {};
template <class X> struct V {};
template <class X> requires C<X> bool operator==(const V<X>&, const U&) { SAY("vt: V,U"); return true; }
template <class X> requires D<X> bool operator==(const U&, const V<X>&) { SAY("vt: U,V"); return true; }
}
int main() {
  t1::L l1;
  ST<Dur> s;
  (void)(l1 < s);
  (void)(l1 >= s);
  t2::L l2;
  (void)(s < l2);
  (void)(s >= l2);
  t3::TA ta;
  t3::TB tb;
  (void)(ta < tb);
  nt::U u;
  nt::W<nt::U> w;
  (void)(u < w);
  (void)(w < u);
  (void)(u == w);
  (void)(w == u);
  (void)(u != w);
  vt::U vu;
  vt::V<int> vv;
  (void)(vv == vu);
  (void)(vu == vv);
  printf("\n");
}
