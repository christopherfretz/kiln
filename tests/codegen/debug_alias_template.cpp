// Debug info (WP-1414): an alias template specialization
// ([temp.alias]) keeps its `DW_TAG_typedef` (clang's
// `UnwrapTypeForDebugInfo` stops at it; `CreateType(const
// TemplateSpecializationType *)`): named by the template's unqualified
// name and its arguments as written (`V<int>`, `A<short>` for
// `n::A<short>`), at the alias declaration, in its scope, of the aliased
// type; a typedef of one keeps both; a function parameter of a
// substituted alias type too. clang 19 and 22 agree.
//@ args: -g
template <class T> struct W { T t; };
template <class T> using V = W<T>;
template <class T> using Ptr = T *;
template <class T> using Id = T;
namespace n {
template <class T, int N = 2> using A = W<T[N]>;
}
struct S {
  template <class T> using M = W<T>;
};
V<int> v;
Ptr<char> p;
Id<long> l;
n::A<short> a;
S::M<float> m;
using Plain = V<double>;
Plain pl;
template <class T> int f(V<T> x) { return sizeof x; }
int use() { return f(V<int>{}); }
