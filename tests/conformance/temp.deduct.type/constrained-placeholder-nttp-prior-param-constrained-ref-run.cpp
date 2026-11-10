// [temp.deduct.type]/20, [dcl.type.auto.deduct]/2, [over.match.class.deduct]/1.3:
// the class template shape of constrained-placeholder-nttp-prior-param-run.cpp
// with a constrained reference (`any auto R`): the origin's placeholder
// type-constraint calls a function of an earlier parameter that is itself
// a constrained placeholder. Copy deduction (`const point q = p;`) and a
// deduced return type must deduce `O` by matching its dependent type, as
// for an unconstrained reference (WP-1235).
//
// clang 19 recurses without bound in `AutoType::Profile` on this shape when
// the unconstrained `auto` it profiles a reference to `R` with (C++20
// [temp.over.link]/6 ignores type-constraints) lands in the folding-set
// bucket of the origin's type, which depends on addresses; it segfaulted
// the x86_64 oracle in merge-group CI now and then (WP-1254). clang 22
// does not. docs/VALIDATION.md, known oracle defects.
//@ std: c++20
//@ requires: clang>=22
//@ expect: run
//@ stdout: unit 7
//@ stdout: ctad 8
//@ stdout: fn 7 9
extern "C" int printf(const char *, ...);

template <class T> concept any = true;

struct spec { int v; };
consteval spec spec_of(spec s) { return s; }
template <class T, auto S> concept origin_for = T::id == S.v;
struct origin { static constexpr int id = 7; };
template <any auto R, origin_for<spec_of(R)> auto O = origin{}, class Rep = double>
struct point {
  Rep value;
  static constexpr auto reference = R;
};

// A deduced return type, then a copy deduction from it.
template <spec S> auto make(int v) { return point<S, origin{}, int>{v}; }

// A function template deducing all three from a `point` argument.
template <any auto R, origin_for<spec_of(R)> auto O, class Rep>
int get(point<R, O, Rep> p) { return R.v * 10 + int(p.value); }

int main() {
  const point<spec{7}, origin{}, int> p{1};
  const point q = p;
  printf("unit %d\n", decltype(q)::reference.v);
  const point s = make<spec{7}>(8);
  printf("ctad %d\n", int(s.value));
  printf("fn %d %d\n", get(p) / 10, get(point<spec{7}, origin{}, int>{9}) % 10);
}
