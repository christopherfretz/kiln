// [temp.variadic]/11, [temp.dep.candidate]/1, [over.match.oper]/3: a
// fold-expression instantiates to the operator expressions that combine
// its elements, and their non-member candidates are those unqualified
// lookup of the operator name found at the fold's definition plus
// argument-dependent lookup at instantiation, as for a dependent binary
// operator. A better operator declared after the definition, outside the
// associated namespaces, is not a candidate (a binary fold with an
// initializer and the left and right unary folds alike); one declared
// later in an associated namespace is found by argument-dependent lookup;
// one found at the definition through a using-declaration is a
// candidate; an empty pack gives the identity value of `&&`, `||` and
// `,` without any lookup.
//@ expect: run
//@ strict-diags
namespace N { struct S { int v; }; }
namespace U { int operator*(N::S a, N::S b) { return a.v * b.v; } }
using U::operator*;
N::S operator-(N::S a, long b) { return {a.v - int(b) * 10}; }
N::S operator+(N::S a, N::S b) { return {a.v + b.v}; }
bool operator&&(N::S, N::S) { return true; }
template <class... T> N::S lfold(N::S init, T... t) { return (init - ... - t); }
template <class... T> N::S rfold(T... t) { return (t + ...); }
template <class... T> N::S lufold(T... t) { return (... + t); }
template <class... T> int mul(T... t) { return (t * ...); }
template <class... T> N::S adl(T... t) { return (t / ...); }
template <class... T> bool all(T... t) { return (t && ...); }
template <class... T> bool any(T... t) { return (... || t); }
template <class... T> void seq(T... t) { (t, ...); }
// Declared after the definitions: better matches (or the only ones) that
// the instantiations do not see.
N::S operator-(N::S a, int b) { return {a.v - b}; }
N::S operator+(N::S &a, N::S &b) { return {1000}; }
bool operator&&(N::S &, N::S &) { return false; }
namespace N {
// Declared after the definitions, found by argument-dependent lookup.
S operator/(S a, S b) { return {a.v / b.v}; }
}
int main() {
  if (lfold(N::S{100}, 1, 2).v != 70)
    return 1;
  if (rfold(N::S{1}, N::S{2}, N::S{3}).v != 6)
    return 2;
  if (lufold(N::S{1}, N::S{2}, N::S{3}).v != 6)
    return 3;
  if (mul(N::S{2}, N::S{3}) != 6)
    return 4;
  if (adl(N::S{12}, N::S{3}).v != 4)
    return 5;
  if (!all(N::S{0}, N::S{0}))
    return 6;
  if (!all() || any())
    return 7;
  seq();
  return lfold(N::S{5}).v == 5 ? 0 : 8;
}
