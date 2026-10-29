// Constructor selection and user-defined conversions (spec/sema-overload.md
// §2.6, §11, WP-068a): a target class with 64 converting constructors, one
// per source class, and 64 source classes that also declare a conversion
// function to an unrelated class, initialized 5,440 times in copy-, direct-
// and list-initialization. Every copy-initialization runs
// [over.match.copy] over the 64 constructors plus the source's conversion
// function; direct- and list-initialization run [over.match.ctor] and
// [over.match.list]. Measures the constructor candidate path, the
// user-defined conversion search and its nested candidate sets. No library
// headers.

#define KILN_8(m, p) m(p##0) m(p##1) m(p##2) m(p##3) m(p##4) m(p##5) m(p##6) m(p##7)
#define KILN_64(m) KILN_8(m, 1) KILN_8(m, 2) KILN_8(m, 3) KILN_8(m, 4) \
                   KILN_8(m, 5) KILN_8(m, 6) KILN_8(m, 7) KILN_8(m, 8)

struct Other { int v; };

#define KILN_SRC(n) \
  struct S##n { \
    int v; \
    constexpr operator Other() const { return {v}; } \
  };
KILN_64(KILN_SRC)

#define KILN_CTOR(n) constexpr Target(const S##n &s) : v(s.v + n) {}
struct Target {
  int v;
  KILN_64(KILN_CTOR)
};

#define KILN_OBJ(n) constexpr S##n s##n{n};
KILN_64(KILN_OBJ)

// 5,440 initializations: 28 rounds of 192 (64 sources, three forms), plus
// 64 more copy-initializations.
#define KILN_INIT(n) \
  { Target a = s##n; Target b(s##n); Target c{s##n}; r += a.v + b.v + c.v; }
#define KILN_ROUND KILN_64(KILN_INIT)
#define KILN_COPY(n) { Target a = s##n; r += a.v; }
int inits() {
  int r = 0;
  KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND
  KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND
  KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND
  KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND
  KILN_64(KILN_COPY)
  return r;
}

// The selections are checked, so a compiler that skips them cannot look
// fast.
struct Yes {};
struct No {};
Yes pick(Target);
No pick(...);
static_assert(__is_same(decltype(pick(s10)), Yes));
static_assert(__is_same(decltype(pick(s87)), Yes));
static_assert(__is_same(decltype(pick(Other{1})), No));
