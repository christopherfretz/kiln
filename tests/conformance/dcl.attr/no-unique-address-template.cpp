// [dcl.attr.nouniqueaddr] (WP-158): a data member's attributes are
// instantiated with it, so a [[no_unique_address]] member of a class
// template specialization may overlap: an empty member takes no storage
// and a member's tail padding holds the following members (run checks the stores do not overlap).
//@ expect: run
//@ strict-diags
struct Empty {};
struct Tail { int a; char b; Tail() : a(1), b(2) {} };
template <class T, class U> struct Pair {
  [[no_unique_address]] T t;
  [[no_unique_address]] U u;
};
template <class T> struct Holder { [[no_unique_address]] T v; char c; };
static_assert(sizeof(Pair<Empty, int>) == sizeof(int));
static_assert(sizeof(Pair<int, Empty>) == sizeof(int));
static_assert(sizeof(Pair<Tail, char>) == 8);
static_assert(sizeof(Holder<Tail>) == 8);
static_assert(sizeof(Holder<Empty>) == 1);
static_assert(__builtin_offsetof(Holder<Tail>, c) == 5);
int main() {
  Pair<Tail, char> p;
  p.u = 3;
  return p.t.b == 2 && p.u == 3 ? 0 : 1;
}
