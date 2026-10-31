// [expr.ref]/4, [basic.lookup.qual.general]/2: the member named by a
// qualified-id after `.`/`->` must be found in the qualifier's scope and
// be a member of the object's class or one of its bases.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:16 error "'U::m' is not a member of class 'D'"
//@ diag: 24:16 error "'N::g' is not a member of class 'D'"
//@ diag: 25:13 error "use of undeclared identifier 'Nope'"
//@ diag: 26:19 error "no member named 'nope' in 'N::B'"
//@ diag: 27:17 error "no member named 'nope' in namespace 'N'"
//@ diag: 28:16 error "'X::Y::z' is not a member of class 'X'"
//@ diag: 29:19 error "'U::m' is not a member of class 'D'"
//@ diag: 33:47 error "'U::m' is not a member of class 'D'"
//@ diag: 34:14 note "in instantiation of function template specialization 'dep<D>' requested here"
//@ diag: 35:47 error "use of undeclared identifier 'Inner'"
//@ diag: 36:14 note "in instantiation of function template specialization 'dep2<D>' requested here"
namespace N { struct B { int m; int f(); }; int g; }
struct D : N::B { int m; typedef N::B Base; };
struct U { int m; };
struct X { struct Y { int z; }; int z; };

int use(D d, D* p, X x) {
  int a = d.U::m;
  int b = d.N::g;
  int c = d.Nope::m;
  int e = d.Base::nope;
  int f = p->N::nope;
  int g = x.Y::z;
  int h = p->::U::m;
  return a + b + c + e + f + g + h;
}

template <class T> int dep(T t) { return t.U::m + t.Inner::v; }
template int dep<D>(D);
template <class T> int dep2(T* t) { return t->Inner::v; }
template int dep2<D>(D*);
