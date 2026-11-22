// The written-reference table (spec/ast.md §18): a concept-id written
// as an expression has a `concept` entry naming the concept, in a
// static_assert, a requires-clause and a requires-expression; and an
// implicit member call of a member function template-id (`s<char>()`
// inside a member) is patched with the specialization overload
// resolution selects. Reduced from real code.
//@ written-refs: concept-ids.refs
template <class T> concept Small = sizeof(T) <= 4;
namespace n {
template <class T> concept Big = !Small<T>;
}
static_assert(Small<int>);
static_assert(n::Big<double[4]>);

template <class T>
  requires Small<T>
int narrow(T t) { return int(t); }

template <class T> constexpr bool both = requires { requires n::Big<T>; };

struct Path {
  template <class C> int s() const { return sizeof(C); }
  template <class C> int s(int k) const { return k; }
  int len() const { return s<char>() + s<wchar_t>(1); }
};
