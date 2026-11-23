// [class.base.init]/2, [temp.variadic]/5: the mem-initializer-id of a pack
// expansion `m(args)...` in a template is looked up when the definition is
// parsed like any other; a name that is neither a member nor a type is
// ill-formed at the definition (clang reports it there). The constructor's
// instantiations report nothing more; a pack of bases still initializes.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:49 error "member initializer 'foo' does not name a non-static data member or base class"
//@ diag: 13:57 error "member initializer 'bar' does not name a non-static data member or base class"

struct A { A(int) {} };
template <class... Ts> struct P : Ts... { P() : foo()... {} };
template <class... Ts> struct Q : Ts... { Q(Ts... ts) : bar(ts)... {} };
template <class... Bs> struct R : Bs... { R(int a) : Bs(a)... {} };
P<> p;
R<A> r(1);
