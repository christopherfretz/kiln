//@ expect: compile-ok
// [temp.names]/5, [temp.arg.template]/1, [temp.dep.type]/6: `X<T>::template
// M` names a member template of a dependent specialization that is not the
// current instantiation; it is a dependent template name, looked up again in
// the specialization `X<T>` becomes. Used as a template template argument
// inside an alias template (libc++ `__basic_common_reference_t`), in a class
// template, and in `typename ...::type` chains.
template <class T> struct X {
  template <class U> using apply = U;
  template <class U> struct M { using type = U; };
};
template <class T, template <class> class F> struct B { using type = F<T>; };

template <class T> using A = typename B<T, X<T>::template apply>::type;
static_assert(__is_same(A<int>, int));

template <class T> using A2 = typename X<T>::template apply<T *>;
static_assert(__is_same(A2<int>, int *));

template <class T> using A3 = typename X<T>::template M<T *>::type;
static_assert(__is_same(A3<int>, int *));

template <class T> using A4 = typename B<T, X<T>::template M>::type::type;
static_assert(__is_same(A4<int>, int));

template <class T> struct Y {
  using type = typename B<T, X<T>::template apply>::type;
};
static_assert(__is_same(Y<long>::type, long));

// Inside `X` itself, `X<T>::template apply` is the current instantiation's
// member ([temp.dep.type]/1).
template <class T> struct Z {
  template <class U> using apply = U *;
  using type = typename B<T, Z<T>::template apply>::type;
};
static_assert(__is_same(Z<int>::type, int *));

// The libc++ form (`__type_traits/common_reference.h`): XREF(A) is
// `__xref<A>::template __apply`, passed to `basic_common_reference`.
template <class From, class To> struct copy_cvref { using type = To; };
template <class From, class To> struct copy_cvref<From &, To> { using type = To &; };
template <class From, class To> struct copy_cvref<const From, To> { using type = const To; };
template <class T> struct xref {
  template <class U> using apply = typename copy_cvref<T, U>::type;
};
template <class, class, template <class> class, template <class> class>
struct basic_common_reference {};
template <template <class> class TQ, template <class> class UQ>
struct basic_common_reference<int, long, TQ, UQ> {
  using type = TQ<UQ<char>>;
};
template <class T, class U>
using basic_common_reference_t =
    typename basic_common_reference<__remove_cvref(T), __remove_cvref(U),
                                    xref<T>::template apply,
                                    xref<U>::template apply>::type;
static_assert(__is_same(basic_common_reference_t<int &, long>, char &));
// TQ = copy cv of `const int`, UQ = copy ref of `long &`: `const (char &)`
// is `char &`.
static_assert(__is_same(basic_common_reference_t<const int, long &>, char &));

// In a requires-expression's type-requirement (libc++'s
// `__common_reference_sub_bullet2`): substitution failure, not an error.
template <class T, class U>
concept has_bcr = requires { typename basic_common_reference_t<T, U>; };
static_assert(has_bcr<int &, long>);
static_assert(!has_bcr<long, int>);
