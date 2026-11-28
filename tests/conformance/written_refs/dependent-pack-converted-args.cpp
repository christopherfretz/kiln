// Written-reference table (spec/ast.md §18.9, §18.14): dependent class
// template-ids with parameter packs or pack expansions convert non-expansion
// non-type arguments to their canonical values while preserving written sugar
// (WP-3689). Redeclarations spelling `V<n::k, Ns...>` / `V<3, Ns...>` and
// `W<T, n::k>` / `W<T, 3>` merge into one function template entity, so
// dependent calls see one candidate and resolve per specialization to that
// entity; written references to `n::k` stay recorded; and keyed partial
// specialization matches carry the converted canonical arguments.
//@ written-refs: dependent-pack-converted-args.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++20
namespace n {
constexpr int k = 3;
}
template <int, int...> struct V {
  static const int v = 0;
};
template <class T, int... Ns> struct W {
  static const int v = 0;
};
template <class T> struct W<T, 3> {
  static const int v = 1;
};

template <int... Ns> int f(V<n::k, Ns...>);
template <int... Ns> int f(V<3, Ns...>) { return 1; }

template <class T> int g(W<T, n::k>);
template <class T> int g(W<T, 3>) { return 2; }

template <int... Ns> int use_f(V<3, Ns...> x) { return f(x); }
template <class T> int use_g(T x) { return g(x); }
template <class T> int use_w() { return W<T, n::k>::v + W<T, 3>::v; }

template <class T> struct Tag {
  static const int v = 0;
};
template <int... Ns> struct Tag<V<n::k, Ns...>> {
  static const int v = sizeof...(Ns);
};

int run() {
  return use_f<1, 2>(V<3, 1, 2>{}) + use_g(W<int, 3>{}) + use_w<int>() + Tag<V<3, 1, 2>>::v;
}
