// [stmt.expand] (P1306R5), extension: an expansion statement whose body
// is not a compound-statement (compound-body.cpp) behaves as if the body
// were braced. Each element's body is instantiated in a block of its own
// (a declaration statement as the body declares a new variable per
// element); `continue` in an `if` body ends the current element and
// `break` leaves the statement; a nested `template for` as the body
// expands per outer element; a dependent body in a function template
// expands when the template is instantiated. Checked at compile time and
// at run time; GCC 16.1 agrees. (`break` is not checked in a
// static_assert: GCC 16.1 wrongly says a constexpr `break` out of an
// expansion statement "flows off the end of the function", braced or
// not; control.cpp checks it with kiln.)
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
//@ stdout: 6 4 4 1 90 3 0 6 3 3
extern "C" int printf(const char*, ...);

constexpr int arr[] = {1, 2, 3};

constexpr int expr_body() {
  int n = 0;
  template for (auto x : {1, 2, 3}) n += x;
  return n;
}

constexpr int if_continue() {
  int n = 0;
  template for (auto x : {1, 2, 3}) if (x == 2) continue; else n += x;
  return n;
}

constexpr int if_break_late() {
  int n = 0;
  template for (constexpr auto x : arr) if (x == 3) break; else n += x + 1;
  return n - 1;
}

constexpr int if_break() {
  int n = 0;
  template for (auto x : {1, 2, 3}) if (x == 2) break; else n += x;
  return n;
}

constexpr int nested() {
  int n = 0;
  template for (auto x : {1, 2, 3})
    template for (auto y : {10, 5}) n += x * y;
  return n;
}

constexpr int decl_body() {
  int n = 0;
  template for (auto x : {1, 2, 3}) int k = (n += x, x);
  return n / 2;
}

constexpr int null_body() {
  int n = 0;
  template for (auto x : {1, 2, 3}) ;
  return n;
}

template <int H>
constexpr int dependent() {
  int n = H;
  template for (constexpr auto r : arr) n += r;
  return n;
}

struct Counter {
  int n = 0;
  constexpr void add(int v) { n += v; }
};

template <class T>
constexpr int member_call() {
  T c;
  template for (constexpr auto r : arr) c.add(r > 1 ? 1 : 1);
  return c.n;
}

template <class... T>
constexpr int pack_body(T... ts) {
  int n = 0;
  template for (auto t : {ts...}) n += t;
  return n;
}

static_assert(expr_body() == 6);
static_assert(if_continue() == 4);
static_assert(nested() == 90);
static_assert(dependent<0>() == 6);
static_assert(member_call<Counter>() == 3);

int main() {
  printf("%d %d %d %d %d %d %d %d %d %d\n", expr_body(), if_break_late(),
         if_continue(), if_break(), nested(), decl_body(), null_body(),
         dependent<0>(), member_call<Counter>(), pack_body(1, 1, 1));
}
