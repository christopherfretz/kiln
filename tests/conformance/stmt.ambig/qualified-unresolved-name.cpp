// [stmt.ambig]/1, [basic.lookup.qual]/1: a statement starting with `S::name`
// where `S` is a non-dependent class or namespace without `name` is clang's
// `ANK_Unresolved`: the declarator decides. `S::name(e);`, `S::name *g;` and
// `S::type d;` are declarations ("no type named 'name' in 'S'"); `S::name = 1;`
// is an expression ("no member named"). A failed qualifier (`Q` undeclared,
// `B<int>` undefined) reports only that failure whatever follows. The same
// holds at namespace scope and in a parameter clause.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 36:3 error "use of undeclared identifier 'Q'"
//@ diag: 37:3 error "use of undeclared identifier 'Q'"
//@ diag: 38:6 error "no type named 'type' in 'S'"
//@ diag: 39:3 error "use of undeclared identifier 'undeclared'"
//@ diag: 42:3 error "implicit instantiation of undefined template 'B<int>'"
//@ diag: 34:27 note "template is declared here"
//@ diag: 43:3 error "implicit instantiation of undefined template 'B<int>'"
//@ diag: 34:27 note "template is declared here"
//@ diag: 44:3 error "implicit instantiation of undefined template 'B<int>'"
//@ diag: 34:27 note "template is declared here"
//@ diag: 45:6 error "no type named 'name' in 'S'"
//@ diag: 46:6 error "no type named 'name' in 'S'"
//@ diag: 47:6 error "no member named 'name' in 'S'"
//@ diag: 48:6 error "no type named 'nf' in namespace 'N'"
//@ diag: 49:6 error "no type named 'nt' in namespace 'N'"
//@ diag: 50:6 error "no type named 'nt' in namespace 'N'"
//@ diag: 51:6 error "no member named 'name' in 'S'"
//@ diag: 52:5 error "no type named 'ut' in the global namespace"
//@ diag: 54:4 error "no type named 'gt' in namespace 'N'"
//@ diag: 55:4 error "no type named 'gt' in 'S'"
//@ diag: 56:12 error "no type named 'pn' in 'S'"
//@ diag: 57:17 error "no type named 'pn' in 'S'"
struct S { int x; };
namespace N { int nv; }
template <class T> struct B;
void f(int e) {
  Q::name(e);
  Q::name *g;
  S::type d;
  undeclared::type a;
}
void h(int e) {
  B<int>::name(e);
  B<int>::name *g;
  B<int>::name & r;
  S::name(e);
  S::name *g2;
  S::name = 1;
  N::nf(e);
  N::nt *p;
  N::nt q;
  S::name[2];
  ::ut *gp;
}
N::gt gv;
S::gt sv;
void a1(S::pn);
void a2(int, S::pn *);
