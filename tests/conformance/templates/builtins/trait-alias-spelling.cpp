// clang TypeTraits.def ALIASes: `__is_literal_type` names the trait
// `__is_literal` and `__is_same_as` names `__is_same`, so diagnostics
// print the aliased trait; `__is_convertible_to` is its own trait.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion failed due to requirement '__is_literal(V)'"
//@ diag: 13:15 error "static assertion failed due to requirement '!__is_same(int, int)'"
//@ diag: 14:15 error "static assertion failed due to requirement '!__is_convertible_to(int, int)'"
//@ diag: 16:17 error "static assertion failed due to requirement '__is_same(float, int)'"
//@ diag: 18:10 note "in instantiation of template class 'A<float>' requested here"
struct V { virtual ~V(); };
static_assert(__is_literal_type(V));
static_assert(!__is_same_as(int, int));
static_assert(!__is_convertible_to(int, int));
template <class T> struct A {
  static_assert(__is_same_as(T, int));
};
A<float> a;
static_assert(__is_literal(int) && __is_same(int, int));
