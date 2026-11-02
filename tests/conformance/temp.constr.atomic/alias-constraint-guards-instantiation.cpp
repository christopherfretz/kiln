// [temp.constr.atomic]/3, [temp.names]/9: substituting into an atomic
// constraint that names a specialization of a constrained alias
// template checks the alias's constraints first; when they are not
// satisfied the substitution fails (the constraint is not satisfied)
// and the aliased class template is never instantiated, so a member
// declaration of it that would be ill-formed is no hard error. This is
// libc++'s `std::projected<I, Proj>` guarding `__projected_impl`
// (ranges_remove_copy.pass.cpp:95; WP-309).
//@ expect: compile-ok
//@ strict-diags
template <class F, class A> struct result {};
template <class A> struct result<int (*)(A), A> { using type = int; };
template <class F, class A> using result_t = typename result<F, A>::type;

template <class F> concept Unary = requires { typename result_t<F, int>; };

template <class F> struct impl {
  struct type {
    using value_type = result_t<F, int>; // ill-formed unless Unary<F>
  };
};
template <Unary F> using projected = typename impl<F>::type;

template <class F> concept HasValue = requires { typename projected<F>::value_type; };
template <class F> concept Callable = HasValue<F> && sizeof(F) > 0;

struct NotCallable {};
static_assert(Callable<int (*)(int)>);
static_assert(!HasValue<NotCallable>);
static_assert(!Callable<NotCallable>);
