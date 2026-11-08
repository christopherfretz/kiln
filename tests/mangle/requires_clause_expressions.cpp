// Mangling differential: the expression forms a requires-clause or a
// dependent `decltype` can hold (spec/abi.md §8.15, WP-818;
// [temp.over.link]/5). Member access through the implicit object
// (`dtfpK_`, implicit access with no base), `this->` (`ptfpT`),
// an `operator->` call (`on pt` with no arity), an overloaded operator
// found by unqualified lookup (`eq`, `de`), functional casts with
// parentheses and braces (`cv`, `tl`), `noexcept` (`nx`), a scalar
// value-initialization (`cv i _E`), and an abominable function type as
// a template argument (`KFiiE`).

template <class T> constexpr bool is_ptr_v = false;
template <class T> constexpr bool is_ptr_v<T*> = true;

struct X {};
bool operator==(X, X);
int operator*(X);

template <class It> struct RI {
  It cur;
  It operator->() const
    requires is_ptr_v<It> || requires(const It i) { i.operator->(); }
  { return cur; }
  int self() const requires requires { this->cur; } { return 0; }
  int bare() const requires requires { cur; } { return 0; }
  int ops() const requires requires(It a, It b) { a == b; *a; } { return 0; }
  int casts() const requires requires { It(); It{}; It(cur); } { return 0; }
  int nothrow() const requires (noexcept(It())) { return 0; }
  int scalar() const requires (sizeof(It) > int()) { return 0; }
};

template <class F> struct W {};
void abominable(W<int(int) const>, W<int(int) volatile &>) {}
struct S { int f(int) const; };
template <class T> void pmf(int (T::*)(int) const) {}

int use() {
  int v = 0;
  RI<int*> r{&v};
  pmf(&S::f);
  return *r.operator->() + r.self() + r.bare() + r.ops() + r.casts() + r.nothrow() + r.scalar();
}

// A non-dependent `sizeof`/`alignof` is the integer literal of its value,
// typed as the integer type it converts to (`Lm4E`, `Li4E`).
template <class T> int sz1() requires (sizeof(T) == sizeof(int)) { return 0; }
template <class T> int sz2() requires (sizeof(T) > 0 && alignof(int) == 4) { return 0; }
template <class T, int N = sizeof(int)> int sz3() requires (N + sizeof(T) > 0) { return 0; }
int use_sizeof() { return sz1<int>() + sz2<int>() + sz3<int>(); }
