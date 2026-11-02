// [expr.const]/10 (C++26, P3068R6), [except.throw], [except.handle],
// [except.ctor]: a throw-expression caught within the same constant
// evaluation is a core constant expression. Handlers match by exact type,
// unambiguous public base, pointer conversion and `...`; unwinding runs
// the destructors of the automatic objects between the throw and the
// handler; `throw;` rethrows the handled exception object.
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: compile-ok
//@ strict-diags

struct Base { int v; };
struct Derived : Base { int w; };
struct Other {};

// Exact type, with the handler's parameter a copy.
constexpr int exact() {
  try {
    throw 41;
  } catch (int e) {
    return e + 1;
  }
  return 0;
}
static_assert(exact() == 42);

// Handlers are tried in order; a non-matching one is skipped.
constexpr int order() {
  try {
    throw Derived{{1}, 2};
  } catch (Other) {
    return -1;
  } catch (const Base &b) {
    return b.v * 10;
  } catch (...) {
    return -2;
  }
  return 0;
}
static_assert(order() == 10);

// Pointer conversions: derived-to-base, qualification, void*.
constexpr int pointers() {
  int r = 0;
  Derived d{{3}, 4};
  try { throw &d; } catch (const Base *b) { r += b->v; }
  int i = 5;
  try { throw &i; } catch (const volatile int *p) { r += 10; }
  try { throw &i; } catch (void *p) { r += *static_cast<int *>(p) * 100; }
  try { throw nullptr; } catch (const int *p) { r += p == nullptr ? 1000 : 0; }
  return r;
}
static_assert(pointers() == 1513);

// catch (...) and rethrow through a reference: the handled object is the
// one rethrown.
constexpr int rethrow() {
  try {
    try {
      throw 1;
    } catch (int &e) {
      e = 7;
      throw;
    }
  } catch (int e) {
    return e;
  }
  return 0;
}
static_assert(rethrow() == 7);

constexpr int catch_all() {
  try {
    throw Other{};
  } catch (...) {
    return 3;
  }
}
static_assert(catch_all() == 3);

// Unwinding destroys the automatic objects of every frame left, in reverse.
struct Logger {
  int *log;
  int id;
  constexpr ~Logger() { *log = *log * 10 + id; }
};
constexpr void inner(int *log) {
  Logger c{log, 3};
  throw Other{};
}
constexpr void middle(int *log) {
  Logger b{log, 2};
  inner(log);
  Logger never{log, 9};
}
constexpr int unwinding() {
  int log = 0;
  try {
    Logger a{&log, 1};
    middle(&log);
  } catch (Other) {
    log = log * 10 + 4;
  }
  return log;
}
static_assert(unwinding() == 3214);

// The exception object is destroyed when its last handler exits.
struct Counted {
  int *live;
  constexpr Counted(int *l) : live(l) { ++*live; }
  constexpr Counted(const Counted &o) : live(o.live) { ++*live; }
  constexpr ~Counted() { --*live; }
};
constexpr int exception_object_destroyed() {
  int live = 0;
  try {
    throw Counted(&live);
  } catch (const Counted &) {
    if (live != 1) return -1;
  }
  return live;
}
static_assert(exception_object_destroyed() == 0);

// An exception caught inside a noexcept function does not leave it.
constexpr int contained() noexcept {
  try {
    throw 5;
  } catch (int e) {
    return e;
  }
}
static_assert(contained() == 5);

// Nested try-blocks: the innermost matching handler wins, and an exception
// thrown in a handler goes to the enclosing try-block.
constexpr int nested() {
  int r = 0;
  try {
    try {
      throw 1;
    } catch (int) {
      r += 1;
      throw 2.0;
    }
  } catch (double d) {
    r += 10;
  }
  return r;
}
static_assert(nested() == 11);

// A function-try-block of an ordinary function: its handler may return.
constexpr int fn_try(int v) try {
  if (v > 0) throw v;
  return 0;
} catch (int i) {
  return i * 2;
}
static_assert(fn_try(0) == 0);
static_assert(fn_try(4) == 8);

// A thrown exception escaping a loop body: the loop's earlier iterations
// left nothing behind.
constexpr int loop() {
  int sum = 0;
  for (int i = 0; i < 10; ++i) {
    try {
      if (i % 2) throw i;
      sum += 100;
    } catch (int e) {
      sum += e;
    }
  }
  return sum;
}
static_assert(loop() == 525);
