// [over.match.best.general]/2.1, /2.8: a non-rewritten candidate wins
// over a rewritten one only when neither has a better conversion for
// some operand; when each is better for one, the use is ambiguous.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:12 error "use of overloaded operator '<' is ambiguous (with operand types 'A' and 'B')"
//@ diag: 15:6 note "candidate function"
//@ diag: 16:22 note "candidate function"
//@ diag: 20:12 error "use of overloaded operator '<' is ambiguous (with operand types 'B' and 'A')"
//@ diag: 16:22 note "candidate function (with reversed parameter order)"
//@ diag: 17:6 note "candidate function"
#include <compare>
struct A {}; struct B {};
struct AA { AA(A) {} }; struct BB { BB(B) {} };
bool operator<(const A&, const BB&);
std::strong_ordering operator<=>(const AA&, const B&);
bool operator<(const BB&, const A&);
bool t1(A a, B b) {
  return a < b;
  return b < a;
}
