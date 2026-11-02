// [temp.pre]/5: the init-declarator-list of a template-declaration or explicit
// specialization contains at most one declarator
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:25 error "a template declaration can only declare a single entity"
//@ diag: 15:29 error "a template declaration can only declare a single entity"
//@ diag: 16:28 error "a template declaration can only declare a single entity"
//@ diag: 19:34 error "a template declaration can only declare a single entity"
//@ diag: 20:30 error "a template declaration can only declare a single entity"
//@ diag: 23:23 error "an explicit template specialization can only declare a single entity"
// The first declarator is declared; the group ends at the error.
template <class T>
T v;
template <class T> int a, b;
template <class T> int c = 1, d = 2;
template <class T> void f(), g();
int x = a<int> + c<long>;
struct S {
  template <class T> static int m, n;
  template <class T> void h(), k();
  int one;
};
template <> int v<int>, v<long>;
