// [expr.cond]/6: arms of different types, one a class, that /4 cannot
// convert to each other are converted by overload resolution among the
// built-in candidates ([over.built]/27-28): `T(T, T)` for a pointer,
// pointer-to-member or scoped enumeration `T`, and `LR(L, R)` for
// promoted arithmetic types (clang `FindConditionalOverload`). A smart
// reference converting to `expr *` meets an `app *` as `expr *` (z3's
// `api_solver.cpp`). WP-799.
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 3|7 2.5|1 9|ce 12
extern "C" int printf(const char *, ...);

struct expr { int id; };
struct app : expr {};

// No constructor from `app *`: /4 converts neither arm.
template <class T> struct ref {
  T *p;
  constexpr ref(T *p, int) : p(p) {}
  constexpr operator T *() const { return p; }
};

struct inum { int v; constexpr operator int() const { return v; } };
struct dnum { double v; constexpr operator double() const { return v; } };

enum class color { red, green, blue };
struct paint { color c; constexpr operator color() const { return c; } };
struct ink { color c; constexpr operator color() const { return c; } };

struct S { int a, b; };
struct mref { int S::*m; operator int S::*() const { return m; } };
struct mref2 { int S::*m; operator int S::*() const { return m; } };

template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };

expr e1{1};
app a2;

expr *pick(const ref<expr> &g, bool b) { return b ? g : &a2; }

static_assert(same<decltype(true ? ref<expr>(nullptr, 0) : (app *)nullptr), expr *>::value);
static_assert(same<decltype(true ? inum{1} : dnum{2}), double>::value);
static_assert(same<decltype(true ? paint{} : ink{}), color>::value);
static_assert(same<decltype(true ? mref{} : mref2{}), int S::*>::value);

constexpr int ce(bool b) {
  double r = b ? inum{5} : dnum{7};
  color c = b ? paint{color::blue} : ink{color::red};
  return int(r) + int(c) + (b ? inum{5} : dnum{0});
}
static_assert(ce(true) == 12);

int main() {
  a2.id = 2;
  ref<expr> g(&e1, 0);
  S s{9, 1};
  mref m{&S::b};
  mref2 m2{&S::a};
  printf("%d %d %d|", pick(g, true)->id, pick(g, false)->id,
         int(true ? paint{color::blue} : ink{color::red}) + 1);
  printf("%d %.1f|", int(true ? inum{7} : dnum{1}), false ? inum{7} : dnum{2.5});
  printf("%d %d|", s.*(true ? m : m2), s.*(false ? m : m2));
  printf("ce %d\n", ce(true));
}
