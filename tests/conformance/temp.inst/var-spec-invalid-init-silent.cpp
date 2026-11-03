// [temp.inst]/7, [dcl.pre]/10, [expr.const]: a variable template
// specialization whose initializer fails to instantiate is invalid; a use of
// it (directly, or through a variable initialized from it) in a
// static_assert or a constexpr initializer reports nothing more (clang's
// error-dependent `DeclRefExpr`). libc++'s is_implicit_lifetime.verify
// expects only the trait error. spec/sema-templates.md "Variable templates".
//@ strict-diags
//@ expect: compile-fail
//@ diag: 19:26 error "incomplete type 'Incomplete' used in type trait expression"
//@ diag: 20:16 note "in instantiation of variable template specialization 'trivial<Incomplete>' requested here"
//@ diag: 17:8 note "forward declaration of 'Incomplete'"
//@ diag: 23:24 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 24:20 note "in instantiation of variable template specialization 'has_x<int>' requested here"
//@ diag: 32:22 error "invalid application of 'sizeof' to an incomplete type 'Incomplete'"
//@ diag: 33:15 note "in instantiation of variable template specialization 'size<Incomplete>' requested here"
//@ diag: 17:8 note "forward declaration of 'Incomplete'"
struct Incomplete;
template <class T>
constexpr bool trivial = __is_trivially_copyable(T);
static_assert(!trivial<Incomplete>);

template <class T>
constexpr bool has_x = T::x;
constexpr bool z = has_x<int>;
static_assert(z);
struct K {
  static constexpr bool w = has_x<int>;
};
static_assert(K::w);

template <class T>
constexpr int size = sizeof(T);
static_assert(size<Incomplete> == 1);
