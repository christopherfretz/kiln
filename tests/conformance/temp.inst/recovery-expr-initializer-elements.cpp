// [temp.inst], [dcl.init.list]/3, [dcl.init.aggr]/5.1, [class.dtor]/15: an
// initialization in a function template that fails where the template is
// defined (its types are not dependent) is diagnosed there, once. Its
// erroneous expression keeps the initialization's elements: a braced list
// (`E e{N}`, the nested `{N, 1}`) and a default member initializer the
// aggregate initialization used. Instantiating the template substitutes
// them again (the list as written, the default member initializer for the
// instantiated member) and adds no diagnostic. Before WP-1740 each
// instantiation added "not yet implemented: substitution into InitListExpr"
// / "CXXDefaultInitExpr".
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:41 error "cannot initialize a variable of type 'E' with an rvalue of type 'int'"
//@ diag: 25:49 error "attempt to use a deleted function"
//@ diag: 19:17 note "'~NoDtor' has been explicitly marked deleted here"
//@ diag: 26:53 error "attempt to use a deleted function"
//@ diag: 19:17 note "'~NoDtor' has been explicitly marked deleted here"
struct A { int x; int y; };
struct NoDtor { ~NoDtor() = delete; };
struct D { int a; const char* s = __builtin_FUNCTION(); NoDtor n; };
struct W { A a; NoDtor n; };
enum E { e0, e1 };

template <int N> void enum_list() { E e{N}; (void)e; }
template <class T> void nsdmi() { D* d = new D{1}; (void)d; }
template <int N> void nested() { W* w = new W{{N, 1}}; (void)w; }
void use() {
  enum_list<1>();
  nsdmi<int>();
  nested<1>();
}
