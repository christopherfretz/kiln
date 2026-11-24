// Function-name spans (spec/ast.md §24.4): value arguments in
// `[P = A, ...]` carry the names the constant printer writes
// (enumerators, variables, members, a class type), a template template
// argument its template, a pack's elements stay at the pack's depth;
// integers and characters get none.
//@ fn-names: bracket-values.names
//@ std: c++20
namespace ns {
enum class Color { red };
int obj;
struct M { int m; };
struct X { int v; };
template <class> struct C {};
template <Color E> void en() { (void)__PRETTY_FUNCTION__; }
template <int *Q> void ptr() { (void)__PRETTY_FUNCTION__; }
template <int M::*PM> void pm() { (void)__PRETTY_FUNCTION__; }
template <X V> void cls() { (void)__PRETTY_FUNCTION__; }
template <template <class> class TT> void tt() { (void)__PRETTY_FUNCTION__; }
template <class... Ts> void pk(Ts...) { (void)__PRETTY_FUNCTION__; }
template <int N, char Ch> void lit() { (void)__PRETTY_FUNCTION__; }
}
void use() {
  ns::en<ns::Color::red>(); ns::ptr<&ns::obj>(); ns::pm<&ns::M::m>(); ns::cls<ns::X{2}>();
  ns::tt<ns::C>(); ns::pk(1, ns::M{}); ns::lit<3, 'x'>();
}
