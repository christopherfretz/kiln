// clang's -Wdeprecated-builtins on a dependent trait: the warning is issued
// where the trait is written and again by each instantiation that rebuilds
// it, under the instantiation note (clang's BuildTypeTrait; sema-templates §9.1).
//@ diag: 10:39 warning "builtin __has_trivial_constructor is deprecated; use __is_trivially_constructible instead"
//@ diag: 10:39 warning "builtin __has_trivial_constructor is deprecated; use __is_trivially_constructible instead"
//@ diag: 11:15 note "in instantiation of variable template specialization 'd<int>' requested here"
//@ diag: 10:39 warning "builtin __has_trivial_constructor is deprecated; use __is_trivially_constructible instead"
//@ diag: 11:25 note "in instantiation of variable template specialization 'd<S>' requested here"
struct S {};
template <class T> constexpr bool d = __has_trivial_constructor(T);
static_assert(d<int> && d<S>);
