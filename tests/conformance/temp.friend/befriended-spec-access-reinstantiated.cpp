// An access in a class template pattern that is granted only because the
// class may instantiate to a befriended specialization is accepted in the
// pattern and re-checked in each instantiation (clang's dependent access
// diagnostics; WP-151). Member function bodies are re-checked when they
// are instantiated.
//@ expect: compile-fail
//@ diag: 22:14 error "'y' is a private member of 'A'"
//@ diag: 20:32 note "implicitly declared private here"
//@ diag: 23:19 error "'f' is a private member of 'A'"
//@ diag: 20:50 note "implicitly declared private here"
//@ diag: 24:20 error "'y' is a private member of 'A'"
//@ diag: 20:32 note "implicitly declared private here"
//@ diag: 25:14 error "'y' is a private member of 'A'"
//@ diag: 20:32 note "implicitly declared private here"
//@ diag: 26:31 error "'y' is a private member of 'A'"
//@ diag: 20:32 note "implicitly declared private here"
//@ diag: 27:24 error "'y' is a private member of 'A'"
//@ diag: 20:32 note "implicitly declared private here"
template <class T> struct B;
class A { static constexpr int y = 2; static int f(); friend struct B<int>; };
template <class T> struct B {
  int z = A::y;
  void g(int = A::f());
  static_assert(A::y == 2);
  int arr[A::y];
  static constexpr int v = A::y;
  int h(A) { return A::y; }
};
B<int> ok;
int ok2 = B<int>::v + ok.h(A());
int s = sizeof(B<char>);
int t = B<char>().h(A());
