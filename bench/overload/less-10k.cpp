// Overloaded relational operators on classes ([over.match.oper],
// [over.built], spec/sema-overload.md §5 and §11, WP-068b): 100 value
// classes, each with a hidden-friend `operator<` (found only by ADL) and a
// conversion to `int`, plus 100 namespace-scope `operator<` overloads for
// unrelated classes that ordinary lookup finds at every use. 10,000
// `a < b` expressions, each building a candidate set of the visible
// non-members, the ADL friend, the rewritten `<=>` lookups and the
// built-in `operator<(L, R)` candidates over the promoted arithmetic types
// the conversion functions reach ([over.built]/13), which is where the
// memoized built-in tables (`sema.ovl.conv_types`) pay off. Mirrors
// mirrorbind's comparison-heavy generated key types. No library headers.

#define KILN_10(m, p) m(p##0) m(p##1) m(p##2) m(p##3) m(p##4) \
                      m(p##5) m(p##6) m(p##7) m(p##8) m(p##9)
#define KILN_100(m, p) KILN_10(m, p##0) KILN_10(m, p##1) KILN_10(m, p##2) \
                       KILN_10(m, p##3) KILN_10(m, p##4) KILN_10(m, p##5) \
                       KILN_10(m, p##6) KILN_10(m, p##7) KILN_10(m, p##8) \
                       KILN_10(m, p##9)

// K0 ... K99: the compared classes; each `<` yields its own result type
// so the selection is checked. U0 ... U99: unrelated overloads.
#define KILN_DECL(n) \
  struct B##n { bool b; }; \
  struct K##n { \
    int v; \
    operator int() const; \
    friend B##n operator<(const K##n &, const K##n &); \
  }; \
  struct U##n { int v; }; \
  bool operator<(const U##n &, const U##n &); \
  K##n k##n;
KILN_100(KILN_DECL, )

// 10,000 comparisons: every class 100 times.
#define KILN_LESS(n) r += (k##n < k##n).b;
int compare() {
  int r = 0;
#define KILN_ROUND KILN_100(KILN_LESS, )
#define KILN_ROUND10 KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND \
                     KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND KILN_ROUND
  KILN_ROUND10 KILN_ROUND10 KILN_ROUND10 KILN_ROUND10 KILN_ROUND10
  KILN_ROUND10 KILN_ROUND10 KILN_ROUND10 KILN_ROUND10 KILN_ROUND10
  return r;
}

// The selections are checked, so a compiler that skips them cannot look
// fast.
static_assert(__is_same(decltype(k00 < k00), B00));
static_assert(__is_same(decltype(k57 < k57), B57));
static_assert(__is_same(decltype(k99 < k99), B99));
