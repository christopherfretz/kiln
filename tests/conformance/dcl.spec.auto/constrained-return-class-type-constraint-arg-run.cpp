// [dcl.type.auto.deduct]/2, [temp.param]/4: a placeholder return type
// whose type-constraint has a class-type constant argument that does not
// depend on the template (`C<tag{3}> auto`) checks the constraint against
// the type deduced from the return statement once the function template
// is instantiated: the argument is converted to the concept's parameter
// first, as a concept-id's is ([temp.arg.nontype]/2). Before WP-1235 kiln
// checked it unconverted ("substitution into constraint expression
// resulted in a non-constant expression").
//@ std: c++20
//@ expect: run
//@ stdout: tag 3
extern "C" int printf(const char *, ...);

struct tag { int v; };
template <auto R, class Rep> struct box {
  static constexpr auto key = R;
  Rep value;
};
template <class B, auto T> concept keyed = B::key.v == T.v;

template <auto R> keyed<tag{3}> auto rebox(box<R, int> b) {
  return box<R, double>{double(b.value)};
}

int main() {
  auto b = rebox(box<tag{3}, int>{1});
  printf("tag %d\n", decltype(b)::key.v + int(b.value) - 1);
}
