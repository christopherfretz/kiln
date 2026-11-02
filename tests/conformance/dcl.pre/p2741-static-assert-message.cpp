// [dcl.pre]/11-12 (C++26, P2741R3): a static_assert-message that is not
// a string literal is an object with `size()` and `data()`; when the
// assertion fails, its characters are evaluated as constant expressions
// and printed. Only what fails is evaluated (the dependent messages of
// templates wait for instantiation).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:15 error "static assertion failed: temporary"
//@ diag: 36:15 error "static assertion failed: in-buf"
//@ diag: 37:15 error "static assertion failed: named"
//@ diag: 38:15 error "static assertion failed: call"
//@ diag: 39:15 error "static assertion failed: prefix"
//@ diag: 40:15 error "static assertion failed: "
//@ diag: 41:15 error "static assertion failed: a<U+0001>z"
//@ diag: 42:15 error "static assertion failed due to requirement '1 + 1 == 3': with requirement"
//@ diag: 42:21 note "expression evaluates to '2 == 3'"
//@ diag: 45:17 error "static assertion failed due to requirement 'sizeof(Buf) == 0': in-buf"
//@ diag: 47:12 note "in instantiation of template class 'Check<Buf>' requested here"
//@ diag: 45:27 note "expression evaluates to '8 == 0'"
struct Msg {
  const char* p;
  unsigned long n;
  constexpr const char* data() const { return p; }
  constexpr unsigned long size() const { return n; }
};
struct Buf {
  char buf[8] = "in-buf";
  constexpr const char* data() const { return buf; }
  constexpr int size() const { return 6; }
};
constexpr Msg make(const char* p, unsigned long n) { return {p, n}; }
constexpr Msg named{"named", 5};
const Msg& ref = named;

static_assert(false, Msg{"temporary", 9});
static_assert(false, Buf{});
static_assert(false, named);
static_assert(false, (make("call", 4)));
static_assert(false, Msg{"prefix-only", 6});
static_assert(false, Msg{nullptr, 0});
static_assert(false, Msg{"a\x01z", 3});
static_assert(1 + 1 == 3, Msg{"with requirement", 16});

template <class T> struct Check {
  static_assert(sizeof(T) == 0, T{});
};
Check<Buf> c1;

template <bool B> struct Holds { static_assert(B, Msg{"unused", 6}); };
Holds<true> h;
static_assert(true, Msg{"never printed", 13});
