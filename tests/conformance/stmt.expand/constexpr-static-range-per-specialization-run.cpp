// [temp.inst], [stmt.expand]/5.3, [class.temporary]/6: a `constexpr`
// destructuring expansion statement over a non-dependent glvalue that
// designates a temporary (`static_cast<const P &&>(P{1, 2})`) binds it by
// a `static constexpr auto&&`, so the temporary has static storage
// duration. Each specialization of the enclosing function template
// extends its own copy: the elements of two specializations designate
// different objects. The pattern's temporary was shared, so the second
// specialization's reference bound a temporary the first had extended,
// which is not a constant expression (WP-3291). As the equivalent
// `static constexpr const P &&r = static_cast<const P &&>(P{1, 2});` does
// in clang 19 and GCC 16.1.
//@ expect: run
//@ strict-diags
//@ skip(clang): clang 22 does not implement expansion statements (and GCC 16.1 rejects the constexpr reference to the static temporary in this form, though not in the equivalent declaration)
//@ stdout: 1 1 2 2 3 3
extern "C" int printf(const char *, ...);
struct P {
  int a, b;
};
template <class T> const int *last() {
  const int *p = nullptr;
  template for (constexpr const int &x : static_cast<const P &&>(P{1, 2})) p = &x;
  return p;
}
template <class T> int sum() {
  int s = 0;
  template for (constexpr int x : static_cast<const P &&>(P{1, 2})) s += x;
  return s;
}
int main() {
  const int *a = last<int>();
  const int *b = last<long>();
  printf("%d %d %d %d %d %d\n", a != b, a == last<int>(), *a, *b, sum<int>(), sum<long>());
}
