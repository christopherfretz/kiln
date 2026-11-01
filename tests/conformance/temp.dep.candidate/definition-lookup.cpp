// [temp.dep.candidate]/1: a dependent unqualified call's candidates are the
// declarations ordinary lookup found at the template's definition, plus
// argument-dependent lookup. Names visible where the instantiation is
// required take no part: the `swap` of namespace `s` is found only through
// ADL, where it is ambiguous with the deleted poison pill of `lib::d`.
//@ expect: compile-ok
namespace lib::d {
template <class T> void swap(T &, T &) = delete;
template <class T> concept C = requires(T &a) { swap(a, a); };
template <class T> constexpr bool v = requires(T &a) { swap(a, a); };
template <class T> int pick(T &a) { return size(a); }
int size(...);
}
namespace s {
template <class P> struct It {};
template <class T> void swap(T &, T &);
template <class T> constexpr bool B = lib::d::C<T>;
template <class T> struct W { static constexpr bool value = lib::d::v<T>; };
struct Q {};
int size(Q &) { return 7; }
}
// First checked from inside an instantiation in namespace `s`.
static_assert(!s::B<s::It<int *>>);
static_assert(!s::W<s::It<char>>::value);
// A namespace-scope name declared after the definition is found by ADL only.
int size(s::Q &);
int main() {
  s::Q q;
  return lib::d::pick(q) == 7 ? 0 : 1;
}
