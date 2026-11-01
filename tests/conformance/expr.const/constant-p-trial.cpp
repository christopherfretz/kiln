// [expr.const] (clang extension __builtin_constant_p) in a constant
// evaluation: the argument is evaluated as a speculative trial
// (spec/oven.md "Trial evaluation"). The trial fails, and the builtin is 0,
// when the argument is not a constant expression, when it modifies an
// object that existed before the trial (other than a heap object), and when
// it allocates or deallocates (clang's speculative evaluation). A failed
// trial leaves no effects; a successful one keeps them.
//@ strict-diags

constexpr int set(int* p) { *p = 7; return 1; }
constexpr int set_fail(int* p) { *p = 7; int z = 0; return 1 / z; }

// (a) A failed trial whose callee writes an object leaves it unchanged,
// whether the callee then fails or not: the caller's object existed before
// the trial.
constexpr int caller_local() {
  int x = 1;
  int r = __builtin_constant_p(set_fail(&x));
  int s = __builtin_constant_p(set(&x));
  return x * 100 + r * 10 + s;
}
static_assert(caller_local() == 100);

// The object under construction existed before the trial too.
struct S {
  int a = 0;
  int b = 0;
  constexpr S() { b = __builtin_constant_p(set(&a)); }
};
constexpr S s;
static_assert(s.a == 0 && s.b == 0);
constexpr int local_s() { S x; return x.a * 10 + x.b; }
static_assert(local_s() == 0);
constexpr int arr[2] = {1, __builtin_constant_p(set(const_cast<int*>(&arr[0])))};
static_assert(arr[0] == 1 && arr[1] == 0);

// A trial changing the active member of a union it did not create.
union U { int i; float f; };
constexpr bool to_float(U* u) { u->f = 1.0f; return true; }
constexpr int union_member() {
  U u{3};
  int r = __builtin_constant_p(to_float(&u));
  return u.i * 10 + r;
}
static_assert(union_member() == 30);

// Objects created by the trial itself may be modified.
constexpr int own_locals() { int y = 1; y = 5; return y; }
constexpr int own_lambda() {
  return __builtin_constant_p([] { int q = 0; set(&q); return q; }());
}
static_assert(__builtin_constant_p(own_locals()) && own_lambda() == 1);

// (b) Allocating in a trial fails it; nothing it allocated is leaked,
// counted or reported afterwards.
constexpr int leaks(int n) {
  int* p = new int[n];
  p[0] = 1;
  return 1;
}
constexpr int balanced() {
  int* q = new int(3);
  int r = *q;
  delete q;
  return r;
}
constexpr int allocating_trials() {
  int r = 0;
  for (int i = 0; i < 100; ++i)
    r += __builtin_constant_p(leaks(1000)) + __builtin_constant_p(balanced());
  return r;
}
static_assert(allocating_trials() == 0);
static_assert(!__builtin_constant_p(balanced()));

// (c) Nested trials: objects of an enclosing trial existed before the
// inner one.
constexpr int inner(int* p) { return __builtin_constant_p(set(p)); }
constexpr int outer() {
  int z = 0;
  int r = inner(&z);
  return r * 10 + z;
}
static_assert(outer() == 0 && __builtin_constant_p(outer()));
constexpr int nest(int* p) {
  int loc = 0;
  int a = __builtin_constant_p(set(&loc));
  *p = 3;
  return a;
}
constexpr int nested_outer_fails() {
  int x = 0;
  int r = __builtin_constant_p(nest(&x));
  return r * 10 + x;
}
static_assert(nested_outer_fails() == 0);
constexpr int nested_untried() {
  int x = 0;
  int r = nest(&x);
  return r * 10 + x;
}
static_assert(nested_untried() == 3);

// (d) A successful trial keeps its effects: heap objects that existed
// before it may be modified.
constexpr int heap_write() {
  int* h = new int(1);
  int r = __builtin_constant_p(set(h));
  int v = *h;
  delete h;
  return v * 10 + r;
}
static_assert(heap_write() == 71);
