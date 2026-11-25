// Per-specialization entries (spec/ast.md §18.9.2, §18.9.4): a generic
// lambda's body is rebuilt by the enclosing function template's
// instantiation. An unqualified call whose argument depends only on the
// enclosing template's parameter (`f(t)`) is no longer dependent there:
// argument-dependent lookup ([temp.dep.candidate]/1) finds `lib::f` and
// the call is resolved by that instantiation, which records its row
// (`gl<lib::X>`), as a non-generic lambda's call is (`gn<lib::X>`). A call
// that still depends on the lambda's own parameter (`f(t, y)`) stays
// dependent: the enclosing instantiation records no row for it, and each
// call operator specialization does (`operator()<long>`); a generic
// lambda that is never called (`unused`) has none. Reduced from real code.
//@ written-refs: inst-adl-generic-lambda.refs
namespace lib {
struct X {};
int f(X);
int f(X, long);
} // namespace lib

template <class T> int gl(T t) {
  auto l = [&](auto y) { return f(t) + (int)sizeof(y); };
  return l(1);
}
int b = gl(lib::X());

template <class T> int gn(T t) {
  auto l = [&](int y) { return f(t) + y; };
  return l(1);
}
int c = gn(lib::X());

template <class T> int gd(T t) {
  auto l = [&](auto y) { return f(t, y); };
  auto unused = [&](auto z) { return f(t, z); };
  return l(1L);
}
int d = gd(lib::X());
