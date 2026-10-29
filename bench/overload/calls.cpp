// Overload resolution of named calls (spec/sema-overload.md §11, WP-068a):
// 200 overloads of one name, each taking a distinct class by const
// reference, called 3,000 times with each argument type in turn, so every
// call builds a 200-candidate set and computes one implicit conversion
// sequence per candidate. The 199 non-matching candidates each need a
// reference binding that tries (and rejects) the parameter class's
// converting constructors, which is where a naive [over.match.ref] /
// [over.match.copy] walk spends its time. Mirrors the shape of
// mirrorbind's generated binding tables (one `bind` overload per bound
// type). Measures candidate-set reuse (§1.4), ICS computation and the
// best-viable scan. No library headers.

#define KILN_10(m, p) m(p##0) m(p##1) m(p##2) m(p##3) m(p##4) \
                      m(p##5) m(p##6) m(p##7) m(p##8) m(p##9)
#define KILN_100(m, p) KILN_10(m, p##0) KILN_10(m, p##1) KILN_10(m, p##2) \
                       KILN_10(m, p##3) KILN_10(m, p##4) KILN_10(m, p##5) \
                       KILN_10(m, p##6) KILN_10(m, p##7) KILN_10(m, p##8) \
                       KILN_10(m, p##9)
#define KILN_200(m) KILN_100(m, ) KILN_100(m, 1)

// S00 ... S199, one overload `Rn bind(const Sn&)` each, and one object
// each.
#define KILN_DECL(n) \
  struct S##n { int v; }; \
  struct R##n { int v; }; \
  R##n bind(const S##n &s); \
  S##n s##n;
KILN_200(KILN_DECL)

// 3,000 calls: every argument type 15 times (spec §11 names 1,000; three
// times that keeps the entry above 0.5 s with clang, spec/bench.md).
#define KILN_CALL(n) r += bind(s##n).v;
int calls() {
  int r = 0;
#define KILN_ROUND KILN_200(KILN_CALL)
  KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND
  KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND
  KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND
  return r;
}

// The selections are checked, so a compiler that skips them cannot look
// fast.
static_assert(__is_same(decltype(bind(s00)), R00));
static_assert(__is_same(decltype(bind(s137)), R137));
static_assert(__is_same(decltype(bind(s199)), R199));
