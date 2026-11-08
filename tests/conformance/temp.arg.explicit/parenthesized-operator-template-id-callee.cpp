// [temp.arg.explicit]/4, [over.call.func]/3, [expr.prim.paren] (WP-699):
// the WP-695 rule for an operator-function-id. A parenthesized
// `(operator()<A>)(x...)`, `(this->operator()<A>)`, `(S::operator()<A>)`,
// `(s.operator()<A>)`, `((s.operator()<A>))` and `(operator+<A>)(x, y)` that
// are called are the same overload set as the unparenthesized template-id:
// the call deduces the trailing parameter pack (and a defaulted parameter)
// instead of naming the specialization with only the explicit or default
// arguments ([over.over]/3).
// Verified against clang 19.
//@ expect: run
//@ stdout: 2 3 6 8 8
extern "C" int printf(const char *, ...);
struct S {
  template <class A, class... T> int operator()(A, T...) const {
    return 1 + static_cast<int>(sizeof...(T));
  }
  int m() const {
    return (operator()<int>)(1, 2, 3) + (this->operator()<int>)(1) +
           (S::operator()<int>)(4, 5);
  }
};
struct V {};
template <class A, class B = int> int operator+(A, B) {
  return static_cast<int>(sizeof(B));
}
int main() {
  S s;
  int a = (s.operator()<int>)(1, 2);
  int b = ((s.operator()<long>))(1, 2, 3);
  int c = (operator+<V>)(V{}, 2.0);
  int d = (::operator+<V>)(V{}, 3.0);
  printf("%d %d %d %d %d\n", a, b, s.m(), c, d);
}
