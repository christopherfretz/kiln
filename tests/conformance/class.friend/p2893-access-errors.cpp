//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 19:58 error "'secret' is a private member of 'C<A, int>'"
//@ diag: 16:7 note "implicitly declared private here"
//@ diag: 20:72 error "'secret' is a private member of 'C<int>'"
//@ diag: 21:17 note "in instantiation of member function 'K<int>::get' requested here"
//@ diag: 16:7 note "implicitly declared private here"
//@ diag: 28:50 error "'z' is a private member of 'E'"
//@ diag: 25:7 note "implicitly declared private here"
// [class.friend]/3 (C++26 P2893), [class.access]/1: only the types a
// friend-type-specifier-list names, and the elements a friend pack
// expansion instantiates with, are befriended.
template <class... Ts> class C {
  friend Ts...;
  int secret = 42;
};
struct A { static int get(const C<A, int>& c) { return c.secret; } };
struct B { static int get(const C<A, int>& c) { return c.secret; } };
template <class T> struct K { static int get(const C<T>& c) { return c.secret; } };
int k = K<int>::get(C<int>{});
struct D;
class E {
  friend A, D;
  int z = 1;
};
struct D { static int get(const E& e) { return e.z; } };
struct F { static int get(const E& e) { return e.z; } };
