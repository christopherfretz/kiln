// [temp.inst]/4, [over.match.list]/1: the deferred in-class initializer of
// a static data member of a class template written `= {v}` stays copy-list-
// initialization when it is instantiated: an explicit constructor is an
// error.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 13:19 error "chosen constructor is explicit in copy-initialization"
//@ diag: 16:15 note "in instantiation of static data member 'S<T>::e' requested here"
//@ diag: 11:21 note "explicit constructor declared here"
struct E { explicit E(int v) : v(v) {} int v; };
template<class X> struct S {
  static inline E e = {X::i};
};
struct T { static constexpr int i = 1; };
int b = S<T>::e.v;
