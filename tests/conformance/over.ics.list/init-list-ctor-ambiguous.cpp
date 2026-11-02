// [over.ics.list]/7, [over.ics.rank]/3.3: converting a braced list to a
// class through an initializer-list constructor and through a two-argument
// constructor of another class are both user-defined conversions, so the
// call is ambiguous (libc++ span from {1, 2} in C++26, P2447R4); within
// one class the initializer-list constructor wins ([over.match.list]/1).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:3 error "call to 'one' is ambiguous"
//@ diag: 32:6 note "candidate function"
//@ diag: 33:6 note "candidate function"
namespace std {
using size_t = decltype(sizeof 0);
template <class E> class initializer_list {
  const E *b;
  size_t n;
public:
  constexpr const E *begin() const { return b; }
  constexpr size_t size() const { return n; }
};
}
struct Pair { Pair(int, int); };
struct Range {
  int k;
  constexpr Range(std::initializer_list<int>) : k(1) {}
  constexpr Range(int, int) : k(2) {}
};
constexpr int three(Range r) { return r.k; }
static_assert(three({1, 2}) == 1 && Range(1, 2).k == 2);
struct Span {
  Span(std::initializer_list<int>);
};
void one(Pair);
void one(Span);
void f() {
  one({1, 2});
}
