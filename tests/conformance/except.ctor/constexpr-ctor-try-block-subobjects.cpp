// [except.ctor], [except.handle] (C++26, P3068R6): an exception
// from a mem-initializer or a subobject constructor reaches the
// constructor's function-try-block handlers after the subobjects already
// constructed are destroyed in reverse order; the handler cannot return
// (it rethrows at its end), so the object never exists. A delegating
// constructor's object is complete once its target returns: an exception
// from its body runs the destructor. Each case records the order of events
// as decimal digits.
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: compile-ok
//@ strict-diags
struct Log {
  long long v = 0;
  constexpr void add(int d) { v = v * 10 + d; }
};

struct M {
  Log *log;
  int id;
  constexpr M(Log *l, int i, bool fail) : log(l), id(i) {
    if (fail) throw i;
    log->add(id);
  }
  constexpr ~M() { log->add(id + 5); }
};

struct Base {
  Log *log;
  constexpr explicit Base(Log *l) : log(l) { log->add(1); }
  constexpr ~Base() { log->add(9); }
};

// Throws from the second of three members: the first is destroyed before
// the handler (which records 4) runs; the third is never constructed.
struct Three : Base {
  M a, b, c;
  constexpr Three(Log *l) try : Base(l), a(l, 2, false), b(l, 3, true), c(l, 4, false) {
    l->add(8);
  } catch (int e) {
    l->add(e == 3 ? 4 : 0);
  }
  constexpr ~Three() { log->add(0); }
};

constexpr long long second_of_three() {
  Log log;
  try {
    Three t(&log);
    log.add(0); // not reached: the handler rethrows
  } catch (int e) {
    log.add(e);
  }
  return log.v;
}
// Base(1), a(2), ~a(7), ~Base(9), handler(4), outer catch(3).
static_assert(second_of_three() == 127943);

// Without a function-try-block the same subobjects are destroyed on the
// way out ([except.ctor]/3).
struct Plain : Base {
  M a, b;
  constexpr Plain(Log *l) : Base(l), a(l, 2, false), b(l, 3, false) { throw 6; }
};

constexpr long long body_throws() {
  Log log;
  try {
    Plain p(&log);
  } catch (int e) {
    log.add(e);
  }
  return log.v;
}
// Base(1), a(2), b(3), ~b(8), ~a(7), ~Base(9), catch(6).
static_assert(body_throws() == 1238796);

// [except.ctor]/4: the delegating constructor's body throws after its
// target constructor completed, so the destructor (5) runs, then the
// members' and base's destructors.
struct Delegating : Base {
  M a;
  constexpr Delegating(Log *l, int) : Base(l), a(l, 2, false) {}
  constexpr Delegating(Log *l) try : Delegating(l, 0) {
    throw 3;
  } catch (int) {
    l->add(4);
  }
  constexpr ~Delegating() { log->add(5); }
};

constexpr long long delegating() {
  Log log;
  try {
    Delegating d(&log);
  } catch (int e) {
    log.add(e);
  }
  return log.v;
}
// Base(1), a(2), ~Delegating(5), ~a(7), ~Base(9), handler(4), catch(3).
static_assert(delegating() == 1257943);

// A destructor's members are destroyed before its function-try-block
// handler is entered, and the handler rethrows at its end
// ([except.handle]). The handler names no member (that is undefined),
// so the log travels in the exception object.
struct DtorTry {
  Log *log;
  M a;
  constexpr DtorTry(Log *l) : log(l), a(l, 2, false) {}
  constexpr ~DtorTry() noexcept(false) try {
    throw log;
  } catch (Log *l) {
    l->add(4);
  }
};

constexpr long long dtor_try() {
  Log log;
  try {
    DtorTry d(&log);
  } catch (Log *l) {
    l->add(6);
  }
  return log.v;
}
// a(2), ~a(7), handler(4), catch(6).
static_assert(dtor_try() == 2746);

int main() {}
