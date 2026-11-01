// [temp.spec.general]/6: the names in the declaration of an explicit
// instantiation or explicit specialization are not access checked, except
// in its function body, initializer, base-clause and member-specification;
// [temp.spec.partial.general]/10: nor are the template arguments of a
// partial specialization's class-head. A reference (`struct Two<X, C::T> *`)
// is checked as usual.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:35 error "'T' is a private member of 'C'"
//@ diag: 42:23 note "implicitly declared private here"
//@ diag: 48:40 error "'T' is a private member of 'C'"
//@ diag: 42:23 note "implicitly declared private here"
//@ diag: 50:45 error "'T' is a private member of 'C'"
//@ diag: 42:23 note "implicitly declared private here"
//@ diag: 51:46 error "'N' is a private member of 'C'"
//@ diag: 42:33 note "implicitly declared private here"
//@ diag: 52:40 error "'N' is a private member of 'C'"
//@ diag: 42:33 note "implicitly declared private here"
//@ diag: 54:42 error "'T' is a private member of 'C'"
//@ diag: 42:23 note "implicitly declared private here"
//@ diag: 55:37 error "'N' is a private member of 'C'"
//@ diag: 42:33 note "implicitly declared private here"
//@ diag: 56:37 error "'T' is a private member of 'C'"
//@ diag: 42:23 note "implicitly declared private here"
//@ diag: 57:28 error "'T' is a private member of 'C'"
//@ diag: 42:23 note "implicitly declared private here"
template <class T> class S {
  typedef int P;
  struct Q { };
  P f(P) { return 0; }
  template <class U> U g();
  static P sv;
  void h();
public:
  static int pub(int);
};
template <> S<char>::P S<char>::f(S<char>::P) { S<char>::Q q; return 0; }
extern template S<int>::P S<int>::f(S<int>::P);
template S<long>::P S<long>::f(S<long>::P);
template <> template <> S<short>::P S<short>::g<S<short>::P>();
template <> S<char>::P S<char>::sv = sizeof(S<char>::Q);
class C { typedef int T; struct N { }; };
template <class X> struct Box { };
template <> struct Box<C::T> { C::T m; };
template struct Box<C::T*>;
template <class X> void tf(X) { }
template void tf<C::T>(C::T);
template <> void tf<C::T*>(C::T*) { C::T z; }
template <class X, class Y> struct Two { };
template <class X> struct Two<X, C::T> { C::T m; };
template <class X> struct Two<X*, C::N> : C::N { };
template <> struct Two<C::N, int> : C::N { };
template <class X> int v = 0;
template <class X> int v<X*> = sizeof(C::T);
template <> int v<C::T> = sizeof(C::N);
template <class X> struct Two<X, C::T> *ref();
void use() { struct Two<C::T, short> *l; }
