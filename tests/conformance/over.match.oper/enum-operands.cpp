// [over.match.oper]/3.3 with [dcl.enum]/5: operands of enumeration type
// select a user-declared operator for the enumeration; inside an
// enumerator list the earlier enumerators have the type of their
// initializer, so only the built-in candidates apply (glibc's `_ISwbit`
// enumerators, which made `<string_view>` report ambiguous operators).
//@ expect: compile-ok
enum E { a = 1, b = 2 };
constexpr E operator|(E, E) { return a; }
constexpr int operator<<(E, int) { return -1; }
static_assert((a | b) == a);
static_assert((b << 1) == -1);
enum F { f0 = 0, f1 = (int)((1UL << f0) << 24), f2 = (1 << f0) | 2, f3 = f1 | f2 };
static_assert(f1 == 1 << 24 && f2 == 3 && f3 == ((1 << 24) | 3));
enum { _ISbit0 = 0, _ISbit = ((_ISbit0) < 8 ? (int)((1UL << (_ISbit0)) << 24) : (int)((1UL << (_ISbit0)) >> 24)) };
static_assert(_ISbit == 1 << 24);
