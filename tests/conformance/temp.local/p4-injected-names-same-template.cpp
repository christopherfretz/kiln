// [temp.local]/4: a lookup that finds the injected-class-names of two
// specializations of one class template (through different bases) is
// not ambiguous when the name is used as a template-name: it names the
// template. Used as a type-name it stays ambiguous.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:10 error "member 'tramp' found in multiple base classes of different types"
//@ diag: 10:23 note "member type 'tramp<0>' found by ambiguous name lookup"
//@ diag: 10:23 note "member type 'tramp<1>' found by ambiguous name lookup"
template <int> struct tramp { int c; };
struct n1 : tramp<0> {};
struct n2 : n1, tramp<1> {
  int f() { return static_cast<tramp<1>&>(*this).c + static_cast<tramp<0>*>(this)->c; }
  using T = tramp<2>;
  void g(tramp* p);
};
