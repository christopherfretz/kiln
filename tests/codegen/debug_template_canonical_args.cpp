// Debug info (WP-1400): a function template specialization's
// `templateParams` describe its canonical converted arguments, as clang's
// `CollectFunctionTemplateParams` does: a defaulted argument written
// through a member typedef of another specialization (the shape of
// libstdc++'s `_RequireAllocator<_Alloc>`) is `alloc<char>`, so
// `enable_if<true, alloc<char> >` is not described; an explicit argument
// named by an alias (in a pack too) and an alias-typed value parameter
// are canonical too.
//@ args: -g
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { typedef T type; };
template <class T> struct alloc {};
using Int = int;
using U = unsigned;
template <class A, class = typename enable_if<true, A>::type> void f(A) {}
template <class T> void g(T) {}
template <U N, class T> void h(T) {}
template <class... Ts> void p(Ts...) {}
void use() {
  f(alloc<char>());
  Int i = 0;
  g<Int>(i);
  h<4>(0);
  p<Int, Int>(i, i);
}
