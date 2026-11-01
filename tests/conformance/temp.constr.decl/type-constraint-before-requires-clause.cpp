// [temp.constr.decl]/3.3: the associated constraints are the conjunction
// of the template parameters' type-constraints, then the requires-clause
// after the template-parameter-list, then the trailing requires-clause.
// Satisfaction short-circuits in that order, so a non-range `R` never
// instantiates `kind<R>` (whose primary template is ill-formed; libc++
// `format_kind` in `formatter<_Rp, _CharT>`). Verified against clang 19.
//@ expect: run
template <class T> concept rng = requires(T& t) { t.begin(); };
template <class R>
constexpr int kind = [] { static_assert(sizeof(R) != sizeof(R), "no"); return 0; }();
template <class T, class C = char> struct fmt;
template <class C> struct fmt<bool, C> { int x = 1; };
template <rng R, class C> requires(kind<R> != 1) struct fmt<R, C> {};
template <rng R> requires(kind<R> != 1) int h(R) { return 10; }
template <class R> int h(R) { return 20; }
template <class T> requires(kind<T> != 1) int k(rng auto) { return 30; }
template <class T> int k(int) { return 40; }
int main() {
  fmt<bool, char> f;
  return f.x + h(true) + k<bool>(1) == 61 ? 0 : 1;
}
