// Function-name spans (spec/ast.md §24.4): return and parameter types are
// printed as the pattern writes them, so an alias or typedef name names
// the alias, an alias template's name the alias template, an
// injected-class-name the pattern, a constrained `auto` the concept;
// the bracketed arguments are canonical.
//@ fn-names: types-sugared.names
//@ std: c++20
namespace ns {
struct S {};
using Alias = S;
typedef S *SPtr;
template <class T> struct W {};
template <class T> using WA = W<T>;
template <class T> concept Any = true;
Alias sug(SPtr, const Alias &) { (void)__PRETTY_FUNCTION__; return {}; }
template <class T> WA<T> at(W<T>) { (void)__PRETTY_FUNCTION__; return {}; }
template <class T> struct C {
  bool eq(const C &) const { (void)__PRETTY_FUNCTION__; return true; }
};
void con(Any auto) { (void)__PRETTY_FUNCTION__; }
}
void use() { ns::sug(nullptr, {}); ns::at(ns::W<ns::Alias>{}); ns::C<ns::Alias>{}.eq({}); ns::con(ns::S{}); }
