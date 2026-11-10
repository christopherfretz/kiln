// [temp.deduct.type]/20, [dcl.type.auto.deduct]/2, [over.match.class.deduct]/1.3:
// a non-type parameter declared with a placeholder whose type-constraint
// names an earlier parameter (`template <auto R, of<R> auto V>`) has a
// dependent type, so deducing it also matches its type against the
// argument's. The placeholder itself deduces nothing (clang's
// `DeduceTemplateArgumentsByTypeMatch` succeeds on `Type::Auto`); the
// constraint is checked once the arguments are substituted. Before WP-1235
// kiln matched `of<R> auto` against `int` exactly and rejected the copy
// deduction candidate (`box b = a;`) and every function template below.
// `point`'s reference is unconstrained here: the constrained form
// (`any auto R`) sets off clang 19's unbounded `AutoType::Profile`
// recursion now and then, so it lives apart, for clang 22 and kiln, in
// constrained-placeholder-nttp-prior-param-constrained-ref-run.cpp (WP-1254).
//@ std: c++20
//@ expect: run
//@ stdout: copy 1 2
//@ stdout: direct 1 2
//@ stdout: fn 3 4
//@ stdout: other 5 6
//@ stdout: unit 7
//@ stdout: ctad 8
extern "C" int printf(const char *, ...);

template <class T, auto V> concept of = sizeof(T) <= sizeof(V) + 8;

template <auto R, of<R> auto V> struct box {
  static constexpr auto r = R;
  static constexpr auto v = V;
};

template <auto R, auto V> struct plain {};

template <auto R, of<R> auto V> int fn(box<R, V>) { return R * 10 + V; }
template <auto R, of<R> auto V> int other(plain<R, V>) { return R * 10 + V; }

// The class template's own shape: a class-type reference and an origin
// whose constraint calls a function of the earlier parameter.
struct spec { int v; };
consteval spec spec_of(spec s) { return s; }
template <class T, auto S> concept origin_for = T::id == S.v;
struct origin { static constexpr int id = 7; };
template <auto R, origin_for<spec_of(R)> auto O = origin{}, class Rep = double>
struct point {
  Rep value;
  static constexpr auto reference = R;
};

// A deduced return type, then a copy deduction from it.
template <spec S> auto make(int v) { return point<S, origin{}, int>{v}; }

int main() {
  box<1, 2> a;
  box b = a;  // the copy deduction candidate
  box c(a);
  printf("copy %d %d\n", int(decltype(b)::r), int(decltype(b)::v));
  printf("direct %d %d\n", int(decltype(c)::r), int(decltype(c)::v));
  printf("fn %d %d\n", fn(box<0, 34>{}) / 10, fn(box<0, 4>{}));
  printf("other %d %d\n", other(plain<0, 56>{}) / 10, other(plain<0, 6>{}));
  const point<spec{7}, origin{}, int> p{1};
  const point q = p;
  printf("unit %d\n", decltype(q)::reference.v);
  const point s = make<spec{7}>(8);
  printf("ctad %d\n", int(s.value));
}
