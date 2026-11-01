// [namespace.udecl]/1, /12: `using typename` names a type member of a
// dependent base; redone at instantiation, where a non-type target is
// an error.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:44 error "'typename' keyword used on a non-type"
//@ diag: 11:67 note "target of using declaration"
//@ diag: 24:58 error "'typename' keyword used on a non-type"
//@ diag: 26:8 note "in instantiation of template class 'F<int>' requested here"
//@ diag: 11:67 note "target of using declaration"
template<class T> struct B { using type = T; static constexpr int value = 1;
  struct In { int v = 7; }; };
template<class T> struct D : B<T> {
  using typename B<T>::type;
  using typename B<T>::In;
  static constexpr type twice(type x) { type y = x; return y + x; }
  type m = 2;
  static constexpr int in() { return In().v; }
};
static_assert(D<int>::twice(3) == 6 && D<int>().m == 2 && D<int>::in() == 7);
static_assert(__is_same(D<long>::type, long));
struct E : B<char> { using typename B<char>::type; type c = 'a'; };
static_assert(__is_same(E::type, char));
template<class T> struct F : B<T> { using typename B<T>::value; };
struct G : B<int> { using typename B<int>::value; };
F<int> fi;
