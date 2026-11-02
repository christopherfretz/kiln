// [class.friend]/11: an unqualified friend function of a local class whose
// name no enclosing block declares gets clang's typo correction over the
// namespace-scope functions: a non-template function, not defined, with
// similar parameters, the name within a third of the friend's in edits,
// ranked by edit distance plus qualifier components ('::' for the global
// namespace; '::Q::' for an enclosing namespace; else the path from the
// common namespace; inline and unnamed namespaces are not spelled). A tie
// suggests nothing, and so does a best candidate the friend could not
// redeclare (return type, parameters, noexcept). A using-declarator's
// target is suggested by its own name (WP-290).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 61:17 error "no matching function 'a1' found in local scope; did you mean '::a1'?"
//@ diag: 42:6 note "'::a1' declared here"
//@ diag: 62:17 error "no matching function 'a2' found in local scope; did you mean '::a2'?"
//@ diag: 43:6 note "'::a2' declared here"
//@ diag: 63:17 error "no matching function found in local scope"
//@ diag: 64:17 error "no matching function 'outer22' found in local scope; did you mean '::outer2'?"
//@ diag: 44:6 note "'::outer2' declared here"
//@ diag: 65:17 error "no matching function found in local scope"
//@ diag: 66:17 error "no matching function found in local scope"
//@ diag: 67:17 error "no matching function 'u' found in local scope; did you mean 'N::u'?"
//@ diag: 47:20 note "'N::u' declared here"
//@ diag: 68:17 error "no matching function 'zz' found in local scope; did you mean 'N::zz'?"
//@ diag: 47:30 note "'N::zz' declared here"
//@ diag: 69:17 error "no matching function found in local scope"
//@ diag: 70:17 error "no matching function 'ww' found in local scope; did you mean 'N::M::ww'?"
//@ diag: 47:69 note "'N::M::ww' declared here"
//@ diag: 71:17 error "no matching function 'anon' found in local scope; did you mean '::anon'?"
//@ diag: 49:18 note "'::anon' declared here"
//@ diag: 72:17 error "no matching function found in local scope"
//@ diag: 73:17 error "no matching function found in local scope"
//@ diag: 74:17 error "no matching function found in local scope"
//@ diag: 75:17 error "no matching function found in local scope"
//@ diag: 76:17 error "no matching function 'infn' found in local scope; did you mean '::infn'?"
//@ diag: 53:27 note "'::infn' declared here"
//@ diag: 77:17 error "no matching function 'vfn' found in local scope; did you mean 'P::vfn'?"
//@ diag: 54:41 note "'P::vfn' declared here"
//@ diag: 78:17 error "no matching function found in local scope"
//@ diag: 79:17 error "no matching function found in local scope"

void a1(int);
void a2(int x);
void outer2();
void outer3(int);
int other();
namespace N { void u(); void zz(); void yy(int); namespace M { void ww(); } }
using N::u;
namespace { void anon(); void anon2(); }
void def() {}
int ret();
void exc() noexcept;
inline namespace I { void infn(char); }
namespace P { inline namespace V { void vfn(); } }
void abcd1();
void abcd2();
namespace X1 { void tie(); }
namespace X2 { void tie(); }
void f() {
  struct L {
    friend void a1(int);
    friend void a2(int);
    friend void outer();
    friend void outer22();
    friend void outer3(long);
    friend void othe();
    friend void u();
    friend void zz();
    friend void yy(long);
    friend void ww();
    friend void anon();
    friend void anon2(int);
    friend void def();
    friend void ret();
    friend void exc();
    friend void infn(char);
    friend void vfn();
    friend void abcde();
    friend void tie();
  };
}
