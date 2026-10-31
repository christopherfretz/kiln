// [over.oper], [expr.const]: overloaded operators in constant evaluation
// (including calls returning a class by reference).
//@ expect: compile-ok
// Overloaded compound assignment and increments in constant evaluation.
struct I {
  int v;
  constexpr I &operator+=(int d) { v += d; return *this; }
  constexpr I &operator++() { ++v; return *this; }
  constexpr I operator++(int) { I t = *this; ++v; return t; }
  constexpr bool operator==(const I &o) const { return v == o.v; }
  constexpr int operator*() const { return v; }
};
constexpr int run() {
  I i{1};
  i += 2;
  ++i;
  I old = i++;
  return *old * 10 + *i;
}
static_assert(run() == 45);
constexpr bool eq() { return I{3} == I{3} && I{3} != I{4}; }
static_assert(eq());
