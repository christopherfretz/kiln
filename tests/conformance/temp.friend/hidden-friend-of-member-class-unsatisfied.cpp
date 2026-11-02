// [temp.friend]/9, [temp.constr.decl]/3, [over.match.viable]/3 (WP-272):
// the enclosing specialization's argument leaves a hidden friend of a
// member class (or member class template) of a class template
// unsatisfied; the call has no viable candidate.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:43 error "invalid operands to binary expression ('view<NoEq>::iterator' and 'view<NoEq>::iterator')"
//@ diag: 19:27 note "candidate function not viable: constraints not satisfied"
//@ diag: 20:16 note "because 'NoEq' does not satisfy 'eq'"
//@ diag: 14:58 note "because 'a == a' would be invalid: invalid operands to binary expression ('const NoEq' and 'const NoEq')"
//@ diag: 30:46 error "invalid operands to binary expression ('view<int>::citer<false>' and 'view<int>::citer<false>')"
//@ diag: 24:27 note "candidate function not viable: constraints not satisfied"
//@ diag: 25:16 note "because 'false' evaluated to false"
template <class T> concept eq = requires(const T& a) { a == a; };
struct NoEq {};
template <class V> struct view {
  struct iterator {
    V v;
    friend constexpr bool operator==(iterator const& x, iterator const& y)
      requires eq<V>
    { return x.v == y.v; }
  };
  template <bool Const> struct citer {
    friend constexpr bool operator==(const citer&, const citer&)
      requires Const
    { return true; }
  };
};
bool a(view<NoEq>::iterator i) { return i == i; }
bool b(view<int>::citer<false> i) { return i == i; }
