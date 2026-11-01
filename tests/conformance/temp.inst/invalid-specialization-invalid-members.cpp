// [temp.inst]/3: a member declaration of a class template specialization
// whose type fails to substitute makes the instantiation fail, but (clang's
// TemplateDeclInstantiator) a field, a typedef or alias, and an enumeration
// with a failing underlying type are still declared, invalid: lookup finds
// them and their uses raise nothing further. A static data member or a member
// function whose type fails is not declared ("no member named"). A failed
// typedef, alias or enumeration invalidates the specialization like a failed
// field (`G<int> g;` is invalid, so `g.f()` instantiates nothing), so does a
// field of a member class whose instantiation fails (`J<int>::N`), and the
// declaration whose qualifier instantiated the failing class (`K<char>::In`)
// goes on with an invalid type rather than a syntax error.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:12 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 47:11 note "in instantiation of template class 'F<int>' requested here"
//@ diag: 39:19 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 40:20 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 41:23 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 42:12 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 48:17 error "no member named 's' in 'F<int>'"
//@ diag: 51:13 error "no member named 'f' in 'F<int>'"
//@ diag: 54:48 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 55:8 note "in instantiation of template class 'G<int>' requested here"
//@ diag: 57:51 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 59:18 note "in instantiation of template class 'H<int>' requested here"
//@ diag: 60:49 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 61:8 note "in instantiation of template class 'I<int>' requested here"
//@ diag: 63:51 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 63:67 note "in instantiation of member class 'J<int>::N' requested here"
//@ diag: 64:8 note "in instantiation of template class 'J<int>' requested here"
//@ diag: 65:40 error "type 'char' cannot be used prior to '::' because it has no members"
//@ diag: 66:1 note "in instantiation of template class 'K<char>' requested here"
//@ diag: 65:40 error "type 'short' cannot be used prior to '::' because it has no members"
//@ diag: 67:1 note "in instantiation of template class 'K<short>' requested here"
//@ diag: 57:71 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 59:29 note "in instantiation of member function 'H<int>::f' requested here"
template <class T> struct F {
  typename T::type m;
  static typename T::type s;
  typedef typename T::type td;
  using al = typename T::type;
  typename T::type f();
  int ok;
};
F<int> *fp;
void use_f() {
  (void)fp->m;
  (void)F<int>::s;
  F<int>::td x;
  F<int>::al y;
  (void)fp->f();
  (void)&F<int>::m;
}
template <class T> struct G { typedef typename T::type td; int k; void f() { T::x(); } };
G<int> g;
void use_g() { g.f(); }
template <class T> struct H { using al = typename T::type; void f() { T::x(); } };
H<int> *hp;
void use_h() { hp->f(); hp->f(); H<int>::al q; }
template <class T> struct I { enum E : typename T::type { e1 }; };
I<int> i;
int ki = I<int>::e1;
template <class T> struct J { struct N { typename T::type m; }; N n; };
J<int> j;
template <class T> struct K { typename T::type m; struct In { int q; }; typedef int Td; };
K<char>::In kin;
K<short>::Td ktd;
int kq = kin.q;
