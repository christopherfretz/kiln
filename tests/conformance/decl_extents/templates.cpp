// The declaration-extent table (spec/ast.md §19.1): a template shares its
// pattern's row; template parameters, member templates defined out of
// line, partial and explicit specializations, deduction guides, alias
// templates, concepts and explicit instantiations have rows; implicit
// declarations and instantiations have none.
//@ decl-extents: templates.extents
//@ std: c++20
template <class T, int N = 3, template <class> class TT = T::template X,
          class... Ps>
struct S {
  template <class U> void f(U);
  struct In {
    int q;
  };
};
template <class T, int N, template <class> class TT, class... Ps>
template <class U>
void S<T, N, TT, Ps...>::f(U) {}
template <class T> struct W {
  W(T);
};
template <class T> W(T) -> W<T>;
template <class T> using Ptr = T *;
template <class T> concept Small = sizeof(T) < 4;
template <class T> T tv = T();
template struct W<int>;
extern template struct W<long>;
W<char> wc(1);
