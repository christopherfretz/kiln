// [temp.res.general], [dcl.decl], [dcl.fct]/2: an error found after substitution is
// reported at the occurrence of the dependent name its declaration wrote
// (spec/sema-templates.md §5.8), also when the declarator writes the type again after
// the declarator-id: a typedef, static data member or variable is found from its
// decl-specifiers (`typedef int (*f)(T::type)`), a trailing return type after its
// `->` even when the last parameter's own type names the same type after its name,
// and a type written twice in one declarator reports each occurrence.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 54:15 error "no type named 'type' in 'X'"
//@ diag: 60:6 note "in instantiation of template class 'A<X>' requested here"
//@ diag: 55:32 error "no type named 'type' in 'X'"
//@ diag: 56:31 error "no type named 'type' in 'X'"
//@ diag: 57:23 error "no type named 'type' in 'X'"
//@ diag: 58:34 error "no type named 'type' in 'X'"
//@ diag: 63:31 error "'type' is a private member of 'P'"
//@ diag: 68:6 note "in instantiation of template class 'B<P>' requested here"
//@ diag: 51:23 note "implicitly declared private here"
//@ diag: 63:53 error "'type' is a private member of 'P'"
//@ diag: 51:23 note "implicitly declared private here"
//@ diag: 64:30 error "'type' is a private member of 'P'"
//@ diag: 51:23 note "implicitly declared private here"
//@ diag: 64:52 error "'type' is a private member of 'P'"
//@ diag: 51:23 note "implicitly declared private here"
//@ diag: 65:36 error "'type' is a private member of 'P'"
//@ diag: 51:23 note "implicitly declared private here"
//@ diag: 65:63 error "'type' is a private member of 'P'"
//@ diag: 51:23 note "implicitly declared private here"
//@ diag: 66:37 error "'type' is a private member of 'P'"
//@ diag: 51:23 note "implicitly declared private here"
//@ diag: 66:58 error "'type' is a private member of 'P'"
//@ diag: 51:23 note "implicitly declared private here"
//@ diag: 71:15 error "'type' is a private member of 'P'"
//@ diag: 76:15 note "in instantiation of function template specialization 'lf<P>' requested here"
//@ diag: 51:23 note "implicitly declared private here"
//@ diag: 72:24 error "'type' is a private member of 'P'"
//@ diag: 51:23 note "implicitly declared private here"
//@ diag: 73:15 error "'type' is a private member of 'P'"
//@ diag: 51:23 note "implicitly declared private here"
//@ diag: 73:37 error "'type' is a private member of 'P'"
//@ diag: 51:23 note "implicitly declared private here"
//@ diag: 74:31 error "'type' is a private member of 'P'"
//@ diag: 51:23 note "implicitly declared private here"
//@ diag: 81:34 error "no type named 'type' in 'X'"
//@ diag: 85:22 note "in instantiation of template class 'Outer<X>::Inner<int>' requested here"
//@ diag: 82:33 error "no type named 'type' in 'X'"
//@ diag: 88:31 error "address of overloaded function 'ft' does not match required type 'int (int (*)(int))'"
//@ diag: 87:25 note "candidate template ignored: substitution failure [with T = P]: 'type' is a private member of 'P'"
struct X {};
class P { typedef int type; };

template <class T> struct A {
  typename T::type a;
  typedef int (*f)(typename T::type);
  static int (*s)(typename T::type);
  typedef typename T::type (*h)(typename T::type);
  using al = int (*)(typename T::type);
};
A<X> ax;

template <class T> struct B {
  auto t(int (*p)(typename T::type)) -> typename T::type;
  auto u(int (*)(typename T::type)) -> typename T::type;
  auto w(int, int (*q)(typename T::type, int)) -> typename T::type;
  auto x(int (*q)(int), typename T::type) -> typename T::type;
};
B<P> bp;

template <class T> void lf() {
  typename T::type x = 0;
  int (*l)(typename T::type) = nullptr;
  typename T::type (*m)(typename T::type) = nullptr;
  static int (*n)(typename T::type);
}
template void lf<P>();

template <class T> struct Outer {
  template <class U> struct Inner {
    U u;
    typedef int (*g)(typename T::type);
    static int (*m)(typename T::type);
  };
};
Outer<X>::Inner<int> oi;

template <class T> auto ft(int (*p)(typename T::type)) -> typename T::type { return 0; }
int (*use_ft)(int (*)(int)) = ft<P>;
