// [temp.over.link]/5-6: a redeclared template head's requires-clause is
// equivalent when its expressions are; a reference to a specialization
// with non-dependent template arguments (`mag<1>`, `f<2>()`) is the
// specialization itself, so the two spellings are equivalent, and
// references to different specializations are not.
//@ verify
template <auto... Ms> struct um {
  friend constexpr bool operator==(um, um) { return true; }
};
template <auto V> constexpr um<V> mag{};
template <int V> constexpr int n = V;
template <int V> constexpr int f() { return V; }
template <class T> concept Mag = true;

template <Mag auto M, class U>
  requires(M != um<>{} && M != mag<1>)
struct su;
template <Mag auto M, class U>
  requires(M != um<>{} && M != mag<1>)
struct su {};

template <class T> requires(sizeof(n<1>) == 4 && f<2>() == 2) struct a;
template <class T> requires(sizeof(n<1>) == 4 && f<2>() == 2) struct a {};

template <class T> requires(n<1> == 1) struct b; // expected-note {{previous template declaration is here}}
template <class T> requires(n<2> == 1) struct b {}; // expected-error {{requires clause differs in template redeclaration}}
