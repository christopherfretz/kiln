// [except.ctor]/1-2, [class.temporary]/4: an exception from inside a
// full-expression destroys the temporaries of that full-expression in
// reverse order of construction, then the enclosing block's automatic
// objects, also in a function with hundreds of expression statements. At
// -O2 kiln moves runs of such statements into internal helper functions
// (WP-641, spec/landed/codegen/WP-641.md); the statements still read and
// write the enclosing function's locals, `this` and lambda captures, unwind through
// its cleanups, and `switch`/`goto` still jump between them. Each case
// compares the event log with the one a loop over the same statements
// produces.
//@ expect: run
//@ args: -O2
//@ strict-diags
int log_[8192];
int n = 0;
int bad = -1;
void note(int v) { log_[n++] = v; }
int check(int v) { if (v == bad) throw v; return v; }

struct T {
  int id;
  T(int i) : id(i) { note(i); }
  T(const T &) = delete;
  ~T() { note(-id); }
};

struct Reg {
  int sum = 0;
  Reg &add(const T &t) { sum += check(t.id); return *this; }
};

#define S1(k) r.add(T(k)).add(T((k) + 5000)), ++count;
#define S4(k) S1(k) S1((k) + 1) S1((k) + 2) S1((k) + 3)
#define S16(k) S4(k) S4((k) + 4) S4((k) + 8) S4((k) + 12)
#define S64(k) S16(k) S16((k) + 16) S16((k) + 32) S16((k) + 48)
#define S256(k) S64(k) S64((k) + 64) S64((k) + 128) S64((k) + 192)

// 300 bindings, 600 expression statements.
int wide(Reg &r) {
  T outer(9000);
  int count = 0;
  S256(1) S16(257) S16(273) S4(289) S4(293) S4(297)
  return count;
}

// The same events, from a loop.
int narrow(Reg &r) {
  T outer(9000);
  int count = 0;
  for (int k = 1; k <= 300; ++k) { S1(k) }
  return count;
}

// A member function: the statements use `this` and a lambda's captures.
struct Owner {
  Reg r;
  int count = 0;
  int wide() {
    T outer(9100);
#undef S1
#define S1(k) r.add(T(k)).add(T((k) + 5000)), ++count;
    S256(1) S64(257)
    auto f = [this, &outer](int k) {
      int seen = 0;
#undef S1
#define S1(k) r.add(T(k)), seen += outer.id;
      S256(k)
      return seen;
    };
    return f(1) + count;
  }
};

// `switch` cases and a `goto` label between the statements.
int jumps(Reg &r, int which) {
  T outer(9200);
  int count = 0;
#undef S1
#define S1(k) r.add(T(k)).add(T((k) + 5000)), ++count;
  switch (which) {
  case 0:
    S256(1)
  case 1:
    S64(257)
    if (count < 100) goto tail;
    S16(321)
  tail:
    S64(337)
    break;
  }
  return count;
}

int logs_equal(int *a, int na, int *b, int nb) {
  if (na != nb) return 0;
  for (int i = 0; i < na; ++i)
    if (a[i] != b[i]) return 0;
  return 1;
}

int saved[8192];

// Runs fn twice: without an exception (or with one at `throw_at`), and
// compares the log and the result with ref's.
template <class F, class G> int same(F fn, G ref, int throw_at) {
  bad = throw_at;
  n = 0;
  int a = -7, b = -7;
  try { a = ref(); } catch (int) { a = -1; }
  int na = n;
  for (int i = 0; i < n; ++i) saved[i] = log_[i];
  n = 0;
  try { b = fn(); } catch (int) { b = -1; }
  return a == b && logs_equal(saved, na, log_, n);
}

int main() {
  static const int throws[] = {-1, 1, 5001, 37, 150, 5150, 299, 5300};
  for (int t : throws) {
    Reg r1, r2;
    if (!same([&] { return wide(r1); }, [&] { return narrow(r2); }, t)) return 2;
    if (r1.sum != r2.sum) return 10;
  }
  // Expected for Owner: the statements in order, then the lambda's.
  {
    bad = -1;
    n = 0;
    Owner o;
    int got = o.wide();
    if (got != 320 + 256 * 9100) return 20;
    int i = 0;
    if (log_[i++] != 9100) return 21;
    for (int k = 1; k <= 320; ++k) {
      if (log_[i++] != k || log_[i++] != k + 5000 || log_[i++] != -(k + 5000) || log_[i++] != -k)
        return 22;
    }
    for (int k = 1; k <= 256; ++k)
      if (log_[i++] != k || log_[i++] != -k) return 23;
    if (log_[i++] != -9100 || i != n) return 24;
  }
  {
    bad = 300;
    n = 0;
    Owner o;
    int got = 0;
    try { got = o.wide(); } catch (int v) { got = -v; }
    if (got != -300 || o.count != 299) return 30;
    // ... T(300) constructed, add throws, T(300) destroyed, then outer.
    if (n < 3 || log_[n - 3] != 300 || log_[n - 2] != -300 || log_[n - 1] != -9100) return 31;
  }
  {
    bad = -1;
    Reg r;
    n = 0;
    if (jumps(r, 0) != 256 + 64 + 16 + 64) return 40;
    n = 0;
    if (jumps(r, 1) != 64 + 64) return 41;
    if (log_[0] != 9200 || log_[1] != 257 || log_[n - 1] != -9200) return 42;
    n = 0;
    if (jumps(r, 2) != 0 || n != 2) return 43;
    bad = 330;
    n = 0;
    int got = 0;
    try { got = jumps(r, 0); } catch (int) { got = -1; }
    if (got != -1 || log_[n - 1] != -9200 || log_[n - 2] != -330 || log_[n - 3] != 330) return 44;
  }
  return 0;
}
