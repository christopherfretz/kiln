// [expr.prim.lambda.closure]/5, [expr.prim.this]/2 (C++23, P0847R7): in
// a lambda with an explicit object parameter that captures `*this` by
// copy, `this` and implicit member access name the captured copy, reached
// through the object parameter. (clang 19 crashes generating code for
// this, hence clang>=22.)
//@ expect: run
//@ std: c++23
//@ strict-diags
//@ requires: clang>=22
struct S {
  int v = 3;
  int star() const {
    auto l = [*this](this auto&& self) { return v + this->v; };
    return l();
  }
  int star_copy() {
    auto l = [*this](this auto self) { return ++v; };
    int r = l();
    return r * 10 + v;
  }
};

int main() {
  S s;
  if (s.star() != 6) return 1;
  if (s.star_copy() != 43) return 2;
  return 0;
}
