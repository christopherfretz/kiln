// Mangling differential (spec/abi.md §8.15, WP-818):
// - a class template to deduce in a dependent functional or list cast
//   is its template name (`cvN1n4viewE_...`, `tlN1n3refE...`; clang
//   `DeducedTemplateSpecializationType` without a deduced type);
// - a specialization of a template template parameter groups its
//   arguments by the parameter's own template head (`T_IJilEE` for a
//   `template <class...> class`), as clang's converted argument list,
//   in a signature and as a dependent qualifier (`srT_IJ...EE5value`).
namespace n {
template <class T> struct view { view(T, int) {} };
template <class T> struct ref { ref(T&) {} };
template <class T> view(T, int) -> view<T>;
struct fn {
  template <class R, class P>
  auto operator()(R&& r, P&& p) const
      -> decltype(view(static_cast<R&&>(r), static_cast<P&&>(p))) {
    return view(static_cast<R&&>(r), static_cast<P&&>(p));
  }
  template <class R> auto all(R& r) const -> decltype(ref{r}) { return ref{r}; }
};
}

template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { typedef T type; };
template <class... T> struct And { static const bool value = true; };
template <class T> struct is_cc { static const bool value = true; };
template <class... T> struct tup {
  template <template <class...> class A = And,
            typename enable_if<A<is_cc<T>...>::value, int>::type = 0>
  tup(const T&...) {}
};

template <class... T> struct L {};
template <class T, class... U> struct M {};
template <template <class...> class A, class... T> void f(A<T...>*, A<int, T...>*) {}
template <template <class, class...> class A, class T> void g(A<T, int>*) {}
template <template <class...> class A> void h(A<int, long>*) {}
template <template <class...> class A, class T> A<T> to(T) { return {}; }

void use() {
  n::fn fo;
  fo(1, 2);
  int i;
  fo.all(i);
  tup<int, long> t(1, 2L);
  L<char>* a = 0;
  L<int, char>* b = 0;
  M<char, int>* c = 0;
  L<int, long>* d = 0;
  f(a, b);
  g(c);
  h(d);
  to<L>(1);
}
