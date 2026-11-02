// [temp.res]/[temp.inst], [dcl.init.list]/5, [expr.assign]/8, [stmt.ranged],
// [stmt.return]: a `std::initializer_list` built from a braced list whose
// elements are only instantiation-dependent (locals of a template) is
// analyzed in the pattern; at instantiation the list initializes the
// object again from the substituted elements: as the right operand of an
// assignment operator function, a call argument, a for-range-initializer,
// a returned value and a variable's initializer. libc++ charconv.msvc
// (`bogus_digits = {static_cast<char>('0' + base), 'A', 'a'};`). WP-296.
//@ expect: compile-ok

#include <initializer_list>

struct V {
  int n = 0, sum = 0;
  constexpr V& operator=(std::initializer_list<char> l) {
    n = l.size();
    for (char c : l) sum += c;
    return *this;
  }
};
constexpr int total(std::initializer_list<int> l) {
  int s = 0;
  for (int x : l) s += x;
  return s;
}
constexpr std::initializer_list<int> pass(std::initializer_list<int> l) { return l; }

template <class T> constexpr int f(T base) {
  V v;
  int b = base;
  v = {static_cast<char>('0' + b), 'A', 'a'};
  std::initializer_list<int> il = {b, b};
  int s = 0;
  for (int x : {b, 2}) s += x;
  return v.n * 1000 + (v.sum - '0' - 'A' - 'a') + int(il.size()) * 100 + s * 10 + total({b, 1});
}
static_assert(f(3) == 3000 + 3 + 200 + 50 + 4);

template <class T> std::initializer_list<int> h(T t) {
  int a = t;
  return pass({a, a});
}

int main() { return h(1).size() == 0; }
