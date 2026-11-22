// The written-reference table (spec/ast.md §18), role `expr`: plain
// names, enumerators, `&f`, `case E:`, lambda captures, operator- and
// literal-operator-ids, template-ids, variable templates, the pack of
// `sizeof...(pack)` ([expr.sizeof]/5), a template or function parameter
// pack. A name has
// one entry where it is written; `enclosing` is the declaration being
// declared for a name in its initializer, and the lambda's call
// operator inside its body.
//@ written-refs: expr-names.refs
enum Color { red, green };
enum class Mode { on, off };
struct V { int x; };
bool operator==(V a, V b) { return a.x == b.x; }
unsigned long long operator""_k(unsigned long long n) { return n * 1000; }
template <class T> T twice(T t) { return t + t; }
template <class T> constexpr T zero = T();
int counter;
int use(int);

int pick(Mode m, Color c) {
  switch (m) {
  case Mode::on:
    return c == red ? 1 : 2;
  case Mode::off:
    break;
  }
  int (*fp)(int) = &use;
  int n = counter + fp(green);
  auto add = [n, &c](int k) { return n + k + c; };
  V a{1}, b{2};
  bool same = operator==(a, b);
  return add(twice<int>(n)) + zero<int> + same + int(operator""_k(2));
}

template <class... Ts> constexpr unsigned long count(Ts... ts) { return sizeof...(Ts) + sizeof...(ts); }
