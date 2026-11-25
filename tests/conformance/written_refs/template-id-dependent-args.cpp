// The written-reference table (spec/ast.md §18.5): a template-id that is
// dependent only through its template arguments has a template name that
// is not dependent ([temp.names]/2, [temp.res.general]/1): lookup at the
// definition resolved it, and the name's entry names the template, as
// the same name with non-dependent arguments does (`n::v<int>`, whose
// entity is the specialization and whose `found=` is the template). The
// forms: qualified and unqualified variable templates, through a
// using-directive; class and alias templates in a type context; a
// variable template-id as a template argument, in a default template
// argument, in `sizeof` and in a requires-expression; nested template-ids
// (each template name has its entry, the inner ones after the outer
// one's qualifier, before the outer name, §18.1). A function template-id
// is an overload set (§18.12): it stays dependent with its found set,
// one element for a single template, two for two overloads. A name after
// a dependent qualifier (`T::template x<int>`) is dependent itself and
// has no definition-context lookup.
//@ written-refs: template-id-dependent-args.refs
namespace m {
template <class T> struct w {};
} // namespace m
namespace n {
template <class T> constexpr int v = 0;
template <class T> struct K {};
template <class T> using A = K<T>;
template <int N> struct I {};
template <class T> int f(T) { return 0; }
template <class T> int f(T, int) { return 0; }
template <class T> int one(T) { return 0; }
template <class T> int u() { return v<T>; }
} // namespace n
namespace d {
using namespace n;
}

template <class T> int g() { return n::v<int> + n::v<T>; }
template <class T> int nested() { return n::v<m::w<T>> + n::v<n::K<m::w<T>>>; }
template <class T> int types() {
  n::K<T> k;
  n::A<T> a;
  n::I<n::v<T>> i;
  return 0;
}
template <class T, int N = n::v<T>> int contexts() {
  return sizeof(n::v<T>) + requires { n::v<T>; } + d::v<T>;
}
template <class T> int calls(T t) { return n::f<T>(t) + n::one<T>(t); }
template <class T> int keyword() { return T::template x<int>; }
struct X {
  template <class U> static constexpr int x = 4;
};
int z = g<int>() + nested<int>() + types<int>() + contexts<int>() + calls(1) +
        keyword<X>() + n::u<int>();
