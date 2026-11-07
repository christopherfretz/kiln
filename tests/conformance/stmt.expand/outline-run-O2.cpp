// [stmt.expand]/2, /5 (P1306R5), [stmt.jump], [except.ctor]/1-2: an
// expansion statement's `break` leaves the whole statement, `continue`
// ends the current element and `return` leaves the function, each after
// destroying the automatic objects in scope in reverse order; an
// exception leaving an element destroys its objects, then the enclosing
// block's. Above a size threshold kiln emits each element into an
// internal helper (WP-793, spec/codegen.md "Expansion-statement
// outlining"; `-fkiln-outline-expansions=1` forces it here): the
// elements still read and write the function's locals, bindings, `this`
// and lambda captures, return class objects (a named return value
// declared in the element included) and nest. Each `template for` runs the
// body of a twin `for` over the same values, and the event logs, results
// and escaping exceptions are compared.
//@ expect: run
//@ args: -O2 -fkiln-outline-expansions=1
//@ strict-diags
//@ requires: clang>=23
//@ stdout: ok 16
#include <initializer_list>
extern "C" int printf(const char *, ...);

int log_[1 << 14];
int n = 0;
int bad = -1;
void note(int v) { log_[n++] = v; }
int check(int v) {
  if (v == bad) throw v;
  return v;
}

struct T {
  int id;
  T(int i) : id(i) { note(i); }
  T(const T &) = delete;
  ~T() { note(-id); }
};

static constexpr int ks[] = {1, 2, 3, 4, 5, 6};

// break, continue, return with a value, a loop and a switch of the element
// with their own break, and an inner expansion statement's break.
#define CONTROL_BODY                                                         \
  {                                                                          \
    T t(k);                                                                  \
    if (k == skip) continue;                                                 \
    if (k == stop) break;                                                    \
    if (k == ret) return sum + 100 * check(k);                               \
    for (int i = 0; i < k; ++i) {                                            \
      if (i == 2) break;                                                     \
      sum += check(i);                                                       \
    }                                                                        \
    switch (k) {                                                             \
    case 1: sum += 1; break;                                                 \
    case 3: { T u(30); sum += check(3); break; }                             \
    default: sum += k;                                                       \
    }                                                                        \
    INNER                                                                    \
    note(sum);                                                               \
  }
#define INNER template for (constexpr int j : {7, 8, 9}) { if (j == 8 + (k & 1)) break; note(j * k); }
int control_x(int stop, int skip, int ret) {
  T outer(1000);
  int sum = 0;
  template for (constexpr int k : ks) CONTROL_BODY
  return sum;
}
#undef INNER
#define INNER for (int j : {7, 8, 9}) { if (j == 8 + (k & 1)) break; note(j * k); }
int control_l(int stop, int skip, int ret) {
  T outer(1000);
  int sum = 0;
  for (int k : ks) CONTROL_BODY
  return sum;
}
#undef INNER

// A class returned in memory; the element's named return value.
struct Big {
  long a[6];
  int tag;
};
#define PICK_BODY                                                            \
  {                                                                          \
    T t(k + 20);                                                             \
    if (k == w) {                                                            \
      Big b{};                                                               \
      b.tag = k * check(k);                                                  \
      b.a[k % 6] = b.tag;                                                    \
      note(b.tag);                                                           \
      return b;                                                              \
    }                                                                        \
    if (k == w + 3) return Big{{1, 2, 3}, -check(k)};                        \
    note(k + 50);                                                            \
  }
Big pick_x(int w) {
  template for (constexpr int k : ks) PICK_BODY
  return Big{{}, -1};
}
Big pick_l(int w) {
  for (int k : ks) PICK_BODY
  return Big{{}, -1};
}

// Locals, a reference parameter, a structured binding, a lambda capturing
// by reference, and `this`, all written by the elements.
struct P {
  int a, b;
};
struct Acc {
  int total = 0;
#define ACC_BODY                                                             \
  {                                                                          \
    T t(k + 10);                                                             \
    add(check(k));                                                           \
    out += x * k + y;                                                        \
    x += 1;                                                                  \
    local[0] += total;                                                       \
    try {                                                                    \
      if (k == 4) throw k * 100;                                             \
      note(check(k + 40));                                                   \
    } catch (int e) {                                                        \
      T c(e);                                                                \
      local[2] += e;                                                         \
    }                                                                        \
  }
#define ACC_PRE                                                              \
  int local[3] = {0, 0, 0};                                                  \
  P p{1, 2};                                                                 \
  auto &[x, y] = p;                                                          \
  auto add = [&](int v) {                                                    \
    local[v % 3] += v;                                                       \
    total += v;                                                              \
  };
  int run_x(int &out) {
    ACC_PRE
    template for (constexpr int k : ks) ACC_BODY
    return local[0] + local[1] + local[2] + out + x + p.b;
  }
  int run_l(int &out) {
    ACC_PRE
    for (int k : ks) ACC_BODY
    return local[0] + local[1] + local[2] + out + x + p.b;
  }
};

// An expansion statement in a lambda (its captures reach the helper) and
// over an aggregate's members; a `void` return.
struct M {
  int a;
  long b;
  short c;
  int d;
};
#define MEM_BODY                                                             \
  {                                                                          \
    T t(int(m) + 60);                                                        \
    acc += check(int(m)) * scale;                                            \
    if (int(m) == stop) return;                                              \
    note(acc);                                                               \
  }
void members_x(M s, int scale, int stop, int &acc) {
  auto f = [&] {
    template for (auto m : s) MEM_BODY
  };
  f();
}
void members_l(M s, int scale, int stop, int &acc) {
  auto f = [&] {
    for (long m : {long(s.a), s.b, long(s.c), long(s.d)}) MEM_BODY
  };
  f();
}

int fails = 0;
int cases = 0;
int la[1 << 14];
template <class F, class G> void compare(F fx, G fl) {
  for (bad = -1; bad < 120; ++bad) {
    if (bad > 10 && bad != 100 && bad != 30) continue;
    n = 0;
    long rx = 0, rl = 0;
    int ex = -2, el = -2;
    try { rx = fx(); } catch (int e) { ex = e; }
    int nx = n;
    for (int i = 0; i < nx; ++i) la[i] = log_[i];
    n = 0;
    try { rl = fl(); } catch (int e) { el = e; }
    bool same = nx == n && rx == rl && ex == el;
    for (int i = 0; same && i < n; ++i) same = la[i] == log_[i];
    if (!same) {
      ++fails;
      printf("case %d bad %d: %ld/%ld %d/%d %d/%d\n", cases, bad, rx, rl, ex, el, nx, n);
    }
  }
  ++cases;
}

int main() {
  for (int stop = 0; stop <= 7; ++stop)
    for (int skip = 0; skip <= 7; skip += 2)
      for (int ret = 0; ret <= 7; ret += 3)
        compare([=] { return control_x(stop, skip, ret); },
                [=] { return control_l(stop, skip, ret); });
  cases = 1;
  for (int w = 0; w <= 7; ++w) {
    auto sum = [](Big b) {
      long s = b.tag;
      for (long v : b.a) s = s * 3 + v;
      return s;
    };
    compare([=] { return sum(pick_x(w)); }, [=] { return sum(pick_l(w)); });
  }
  compare([] { Acc a; int o = 5; long r = a.run_x(o); return r * 1000 + o + a.total; },
          [] { Acc a; int o = 5; long r = a.run_l(o); return r * 1000 + o + a.total; });
  for (int stop = 0; stop <= 5; ++stop)
    compare([=] { int acc = 1; members_x(M{1, 2, 3, 4}, 7, stop, acc); return acc; },
            [=] { int acc = 1; members_l(M{1, 2, 3, 4}, 7, stop, acc); return acc; });
  if (fails == 0) printf("ok %d\n", cases);
  return fails != 0;
}
