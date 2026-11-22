// [expr.const]/10, [implimits]: a fault in the body of a small leaf callee
// the Oven inlines at its call sites (spec/oven.md "Inlining") is
// diagnosed as in a call: the same note, an "in call to" note for the
// inlined call with its argument values (a parameter the body changed at
// its current value), and the depth limit at the inlined call with its
// backtrace. The first execution of a call site is a real call (it
// inlines the callee), so each fault comes on a later one.
//@ args: -fconstexpr-depth=20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 52:15 error "static assertion expression is not an integral constant expression"
//@ diag: 42:43 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 48:10 note "in call to 'a.get(4)'"
//@ diag: 52:15 note "in call to 'sum_arr(5)'"
//@ diag: 61:15 error "static assertion expression is not an integral constant expression"
//@ diag: 53:53 note "division by zero"
//@ diag: 58:10 note "in call to 'quot(&x, 0)'"
//@ diag: 61:15 note "in call to 'divs()'"
//@ diag: 70:15 error "static assertion expression is not an integral constant expression"
//@ diag: 62:44 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 67:10 note "in call to 'deref(&a[3])'"
//@ diag: 70:15 note "in call to 'reads()'"
//@ diag: 76:15 error "static assertion expression is not an integral constant expression"
//@ diag: 73:10 note "constexpr evaluation exceeded maximum depth of 20 calls"
//@ diag: 73:33 note "in call to 'deep(one, 11)'"
//@ diag: 73:33 note "in call to 'deep(one, 12)'"
//@ diag: 73:33 note "in call to 'deep(one, 13)'"
//@ diag: 73:33 note "in call to 'deep(one, 14)'"
//@ diag: 73:33 note "in call to 'deep(one, 15)'"
//@ diag: 73:33 note "(skipping 10 calls in backtrace; use -fconstexpr-backtrace-limit=0 to see all)"
//@ diag: 73:33 note "in call to 'deep(one, 26)'"
//@ diag: 73:33 note "in call to 'deep(one, 27)'"
//@ diag: 73:33 note "in call to 'deep(one, 28)'"
//@ diag: 73:33 note "in call to 'deep(one, 29)'"
//@ diag: 76:15 note "in call to 'deep(one, 30)'"
//@ diag: 92:15 error "static assertion expression is not an integral constant expression"
//@ diag: 81:12 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 89:10 note "in call to 'next_at(&a[0], 3)'"
//@ diag: 92:15 note "in call to 'walk()'"
struct Arr {
  int v[4];
  constexpr int get(int i) const { return v[i]; }
};
constexpr int sum_arr(int n) {
  Arr a{{1, 2, 3, 4}};
  int s = 0;
  for (int i = 0; i < n; ++i)
    s += a.get(i);
  return s;
}
static_assert(sum_arr(4) == 10);
static_assert(sum_arr(5) == 15);
constexpr int quot(const int *p, int d) { return *p / d; }
constexpr int divs() {
  int x = 12;
  int s = 0;
  for (int d = 3; d >= 0; --d)
    s += quot(&x, d);
  return s;
}
static_assert(divs() == 0);
constexpr int deref(const int *p) { return *p; }
constexpr int reads() {
  int a[3] = {1, 2, 3};
  int s = 0;
  for (const int *p = a; p <= a + 3; ++p)
    s += deref(p);
  return s;
}
static_assert(reads() == 6);
constexpr int val(const int &r) { return r; }
constexpr int deep(const int &r, int n) {
  return val(r) + (n == 0 ? 0 : deep(r, n - 1));
}
constexpr int one = 1;
static_assert(deep(one, 30) == 31);
static_assert(deep(one, 18) == 19);
constexpr int next_at(const int *p, int i) {
  i += 1;
  if consteval {
    return p[i];
  }
  return 0;
}
constexpr int walk() {
  int a[3] = {1, 2, 3};
  int s = 0;
  for (int i = 0; i < 3; ++i)
    s += next_at(a, i);
  return s;
}
static_assert(walk() == 5);
