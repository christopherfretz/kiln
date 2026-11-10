// [temp.spec.partial.match]/2, [temp.deduct.type]/20, [temp.param]/6,
// [dcl.type.class.deduct]/2, [over.match.class.deduct]/1.3: a non-type
// parameter typed by a deduced class template placeholder (`template <fs F>`)
// binds a deduced value whose type is a specialization of that template
// (class template argument deduction from the value picks the copy deduction
// candidate), in a partial specialization, beside a type argument, and in a
// function template; a value of another template's type does not bind it,
// and `fs<N> F` is more specialized than `fs F`. Before WP-1217 kiln compared
// the placeholder itself with `fs<3>` and matched none of them (a WP-1105
// regression: glaze's `to<JSON, float_format_t<Fmt, T>>` stopped matching).
//@ std: c++20
//@ expect: run
//@ stdout: A 1 3
//@ stdout: B 4
//@ stdout: C 1 2 0
//@ stdout: D 4 0
//@ stdout: E 1 0
//@ stdout: G 2
//@ stdout: g 5 0
using size_t = decltype(sizeof 0);
extern "C" int printf(const char *, ...);
template <size_t N> struct fs {
  char v[N]{};
  constexpr fs(const char (&s)[N]) { for (size_t i = 0; i < N; ++i) v[i] = s[i]; }
};
template <size_t N> struct gs {
  char v[N]{};
  constexpr gs(const char (&s)[N]) { for (size_t i = 0; i < N; ++i) v[i] = s[i]; }
};
template <fs F> struct ff {};
template <auto V> struct fa {};
template <fs F, class T> struct ft {};

// The minimal shape: a placeholder-typed parameter deduced from the value.
template <class T> struct A { static constexpr int v = 0; };
template <fs F> struct A<ff<F>> { static constexpr int v = 1 + F.v[0] - 'a'; };
// The same through an explicitly typed `fs<N>` parameter.
template <class T> struct B { static constexpr int v = 0; };
template <size_t N, fs<N> F> struct B<ff<F>> { static constexpr int v = N; };
// An `auto` parameter deduced from a class-type value.
template <class T> struct C { static constexpr int v = 0; };
template <auto V> struct C<fa<V>> { static constexpr int v = 1; };
template <auto V> struct C<ff<V>> { static constexpr int v = 2; };
// The placeholder in a second argument, beside a type (to<10, ff<F, T>>).
template <int I, class T> struct D { static constexpr int v = 0; };
template <fs F, class T> struct D<10, ft<F, T>> { static constexpr int v = sizeof(T); };
// A value of another class template's type does not bind `fs F`.
template <class T> struct E { static constexpr int v = 0; };
template <fs F> struct E<fa<F>> { static constexpr int v = 1; };
// Partial ordering: `fs<N> F` is more specialized than `fs F`.
template <class T> struct G { static constexpr int v = 0; };
template <fs F> struct G<ff<F>> { static constexpr int v = 1; };
template <size_t N, fs<N> F> struct G<ff<F>> { static constexpr int v = 2; };
// A function template deducing the placeholder-typed parameter.
template <fs F> int g(ff<F>) { return sizeof(F.v); }
int g(...) { return 0; }

int main() {
  printf("A %d %d\n", A<ff<"ab">>::v, A<ff<"c">>::v);
  printf("B %d\n", B<ff<"abc">>::v);
  printf("C %d %d %d\n", C<fa<fs{"ab"}>>::v, C<ff<"ab">>::v, C<int>::v);
  printf("D %d %d\n", D<10, ft<"x", int>>::v, D<9, ft<"x", int>>::v);
  printf("E %d %d\n", E<fa<fs{"ab"}>>::v, E<fa<gs{"ab"}>>::v);
  printf("G %d\n", G<ff<"ab">>::v);
  printf("g %d %d\n", g(ff<"abcd">{}), g(fa<1>{}));
}
