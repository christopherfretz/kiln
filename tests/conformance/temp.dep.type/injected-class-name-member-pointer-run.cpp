// [temp.dep.type]/1, [temp.local]/1, [dcl.mptr]/1, [expr.unary.op]/3,
// [expr.mptr.oper]/4, [class.qual]/1: inside a class template the
// injected-class-name `Scan` as the nested-name-specifier of a
// member-pointer declarator (`void (Scan::*)()`) and of `&Scan::f` names
// the current instantiation, so after instantiation `this->*fn` calls the
// member of `Scan<char>`; and `Base::m` looks `m` up in `Base` only, never
// finding the derived class's member function `m()`. The shape of
// libstdc++'s `_Scanner<_CharT>` (`_M_eat_escape`) and `_State<_CharT>`
// (`_M_opcode()`) in <regex> (WP-608).
//@ expect: run
//@ strict-diags
//@ stdout: ecma posix 7 9
#include <cstdio>

struct Base {
  int _M_opcode;
};

template <class C> struct Scan : Base {
  typedef void (Scan::*Fn)();
  Fn fn;
  bool ecma;
  Scan(bool e) : Base{e ? 7 : 9}, ecma(e) { fn = e ? &Scan::eat_ecma : &Scan::eat_posix; }
  void eat_ecma() { std::printf("ecma "); }
  void eat_posix() { std::printf("posix "); }
  void run() { (this->*fn)(); }
  int _M_opcode() const { return Base::_M_opcode; }
};

int main() {
  Scan<char> a(true);
  Scan<wchar_t> b(false);
  a.run();
  b.run();
  std::printf("%d %d\n", a._M_opcode(), b._M_opcode());
  if (a.fn != &Scan<char>::eat_ecma)
    return 1;
  if (b.fn != &Scan<wchar_t>::eat_posix)
    return 2;
  return a._M_opcode() == 7 && b._M_opcode() == 9 ? 0 : 3;
}
