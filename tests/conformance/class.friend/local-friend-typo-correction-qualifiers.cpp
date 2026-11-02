// [class.friend]/11, typo correction for a local class's friend: the
// suggested function is qualified from the namespace enclosing the
// function; an enclosing namespace is spelled from the global namespace
// ('::Q::'), a sibling by its path from the common namespace ('R::',
// 'B::'). Each component costs more than a character edit (WP-290).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:28 error "cannot define friend function 'abcdxf' in a local class definition; did you mean '::Q::abcdef'?"
//@ diag: 19:20 note "'::Q::abcdef' declared here"
//@ diag: 23:50 error "cannot define friend function 'abcdefghx' in a local class definition; did you mean 'R::abcdefghi'?"
//@ diag: 20:20 note "'R::abcdefghi' declared here"
//@ diag: 30:28 error "cannot define friend function 'bfun' in a local class definition; did you mean 'B::bfun'?"
//@ diag: 26:34 note "'B::bfun' declared here"
//@ diag: 30:51 error "cannot define friend function 'bothex' in a local class definition; did you mean 'B::bother'?"
//@ diag: 27:20 note "'B::bother' declared here"
//@ requires: clang>=22
// kiln at -fclang-compat=22 (WP-377); clang 19 wording: "no matching function 'abcdxf' found in local scope; did you mean '::Q::abcdef'?"

namespace Q { void abcdef(); }
namespace R { void abcdefghi(); }
namespace Q {
  void k() {
    struct L { friend void abcdxf(); friend void abcdefghx(); };
  }
}
namespace A { namespace B { void bfun(int); } }
namespace B { void bother(); }
namespace A {
  void m() {
    struct L { friend void bfun(int); friend void bothex(); };
  }
}
