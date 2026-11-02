// [temp.inst]/5, [expr.prim.req.general]/5, [stmt.ranged]/1: a member
// function instantiated while a range-based `for` builds `__begin != __end`
// (here the reversed `operator==` of a sentinel class template) keeps its
// own substitution contexts: a requires-expression whose requirement is
// invalid is false, not an error, and a hard error in the instantiated body
// carries its "in instantiation of" note. libc++ test support
// `sentinel_wrapper::operator==` (test_iterators.h) under `insert_range`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 65:11 error "no member named 'missing' in"
//@ diag: 71:14 note "in instantiation of member function 'Bad<It>::operator==' requested here"

struct It {
  int* p;
  constexpr int operator*() const { return *p; }
  constexpr It& operator++() { ++p; return *this; }
};
struct Counted {
  int* p;
  int* n;
  constexpr int operator*() const { return *p; }
  constexpr Counted& operator++() { ++p; return *this; }
  constexpr void record() const { ++*n; }
};

template <class I> struct Sent {
  int* end;
  constexpr bool operator==(const I& other) const {
    if constexpr (requires { other.record(); }) {
      other.record();
    }
    return other.p == end;
  }
};
template <class I> struct Range {
  I b;
  Sent<I> e;
  constexpr I begin() const { return b; }
  constexpr Sent<I> end() const { return e; }
};

void plain(Range<It>& r) {
  for (int x : r) { (void)x; }
}

constexpr int count() {
  int a[3] = {1, 2, 3};
  int n = 0;
  int sum = 0;
  for (int x : Range<Counted>{{a, &n}, {a + 3}}) { sum += x; }
  return n * 10 + sum;
}
static_assert(count() == 46);

constexpr bool simple() {
  int a[2] = {4, 5};
  int sum = 0;
  for (int x : Range<It>{{a}, {a + 2}}) { sum += x; }
  return sum == 9;
}
static_assert(simple());

template <class I> struct Bad {
  bool operator==(const I& other) const {
    other.missing();
    return true;
  }
};
struct BadRange { It begin(); Bad<It> end(); };
void bad(BadRange& r) {
  for (int x : r) { (void)x; }
}
