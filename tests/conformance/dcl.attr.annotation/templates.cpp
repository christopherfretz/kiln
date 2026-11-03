//@ expect: ast
//@ skip(clang): clang 19 does not implement P3394 annotations
//@ oracle: none
// Annotations are instantiated with their entity ([dcl.attr.annotation]/3:
// each instantiation is a new annotation); a value-dependent operand is
// kept unevaluated in the pattern. `= e...` expands a pack into one
// annotation per element ([dcl.attr.grammar]/6). A function template
// specialization substitutes its annotations when it is used, never
// during deduction ([dcl.attr.annotation]/4, spec/annotations.md §5).
template <int N> struct [[=N]] C {
  [[=N + 1]] int field;
  [[=N + 2]] static int s;
  [[=N + 3]] void method([[=N + 4]] int arg);
  template <int M> [[=N + M]] void member_template();
};
C<10> c;
void use_members() { c.method(0); c.member_template<5>(); }
template <int... Is> [[=Is...]] [[=(Is + ...) == 6]] int packed = 0;
int p = packed<1, 2, 3>;
template <class T> [[=T::value]] void fn([[=T::value + 1]] T);
struct V { static constexpr int value = 7; };
void use_fn() { fn(V{}); }
template <class T> struct [[=1]] P { };
template <class T> struct [[=2]] P<T *> { };
template <> struct [[=3]] P<int> { };
P<char *> p1;
P<int> p2;
P<char> p3;
