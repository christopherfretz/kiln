// [declval]: a declval-style function declared but never defined, used in
// unevaluated operands, yields `T&&` (an lvalue reference for `T&`).
//@ strict-diags
template <class T> struct Decl {
  static T &&get() noexcept;
};
struct S { int m; double f() const; };
static_assert(__is_same(decltype(Decl<int>::get()), int &&));
static_assert(__is_same(decltype(Decl<int &>::get()), int &));
static_assert(__is_same(decltype(Decl<S>::get().m), int));
static_assert(__is_same(decltype((Decl<S>::get().m)), int &&));
static_assert(__is_same(decltype(Decl<const S &>::get().f()), double));
static_assert(noexcept(Decl<S>::get()));
static_assert(sizeof(Decl<S>::get()) == sizeof(S));
