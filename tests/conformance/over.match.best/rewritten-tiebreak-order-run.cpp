// [over.match.best.general]/2: a rewritten or reversed comparison
// candidate ([over.match.oper]/3.4) loses only the late tie-breakers
// /2.8 (non-rewritten over rewritten) and /2.9 (non-reversed over
// reversed). The earlier criteria decide first: a better implicit
// conversion sequence (/2.1, compared in parameter order for a reversed
// candidate), a non-template over a template specialization (/2.4) and
// the more specialized template (/2.5). Each operator prints its name.
//@ expect: run
//@ stdout: a: <=>|b: <=>|c: <=>|d: reversed <=>|e: >=|f: S,T|f: T,S|
#include <compare>
extern "C" int printf(const char*, ...);
#define SAY(s) printf("%s|", s)
namespace a { // /2.4: a non-template rewritten beats a template non-rewritten
struct S {};
template <class T> bool operator<(const S&, const T&) { SAY("a: template <"); return true; }
std::strong_ordering operator<=>(const S&, const S&) { SAY("a: <=>"); return std::strong_ordering::less; }
}
namespace b { // /2.1: a better conversion beats a non-rewritten candidate
struct S {};
struct L { L(const S&) {} };
bool operator<(const S&, const L&) { SAY("b: <"); return true; }
std::strong_ordering operator<=>(const S&, const S&) { SAY("b: <=>"); return std::strong_ordering::less; }
}
namespace c { // /2.5: a more specialized rewritten template beats a non-rewritten one
template <class T> struct P {};
template <class T, class U> bool operator<(const T&, const U&) { SAY("c: <"); return true; }
template <class T> std::strong_ordering operator<=>(const P<T>&, const P<T>&) { SAY("c: <=>"); return std::strong_ordering::less; }
}
namespace d { // /2.1 for a reversed candidate: its operands are (y, x)
struct X {};
struct Y {};
struct Z { Z(const Y&) {} };
bool operator<(const Z&, const X&) { SAY("d: <"); return true; }
std::strong_ordering operator<=>(const X&, const Y&) { SAY("d: reversed <=>"); return std::strong_ordering::less; }
}
namespace e { // /2.8: otherwise equal, the non-rewritten candidate
struct S {};
bool operator>=(const S&, const S&) { SAY("e: >="); return true; }
std::strong_ordering operator<=>(const S&, const S&) { SAY("e: <=>"); return std::strong_ordering::less; }
}
namespace f { // /2.9: of two rewritten candidates, the non-reversed one
struct S {};
struct T {};
std::strong_ordering operator<=>(const S&, const T&) { SAY("f: S,T"); return std::strong_ordering::less; }
std::strong_ordering operator<=>(const T&, const S&) { SAY("f: T,S"); return std::strong_ordering::less; }
}
int main() {
  a::S as;
  (void)(as < as);
  b::S bs;
  (void)(bs < bs);
  c::P<int> cp;
  (void)(cp < cp);
  d::X dx;
  d::Y dy;
  (void)(dy < dx);
  e::S es;
  (void)(es >= es);
  f::S fs;
  f::T ft;
  (void)(fs < ft);
  (void)(ft > fs);
  printf("\n");
}
