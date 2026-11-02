// [class.friend]/2, [class.access]/1: friendship granted by a class nested
// in a class template specialization D<T> is granted to D<T> (and so to
// the member declarations of D<T>, a sibling member class included) only:
// another specialization and an unrelated class still get the access error.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:48 error "'t_' is a private member of"
//@ diag: 14:7 note "implicitly declared private here"
//@ diag: 18:74 error "'t_' is a private member of"
//@ diag: 25:41 note "in instantiation of function template specialization 'D<long>::other<int>'"
//@ diag: 14:7 note "implicitly declared private here"
template <class T> struct D {
  class P {
    T t_ = 1;
    friend class D;
  };
  struct Q { T get(const P &p) const { return p.t_; } };
  template <class U> T other(const typename D<U>::P &p) const { return p.t_; }
};
struct Unrelated {
  int get(const D<int>::P &p) const { return p.t_; }
};
int main() {
  D<int>::P p;
  return D<int>::Q().get(p) + D<long>().other<int>(p);
}
