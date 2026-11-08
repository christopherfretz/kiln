// [class.temporary]/4, [dcl.init.list]/3.7 with CWG2327: in `S{tl}` the
// result of `TL::operator S()` initializes the object directly, and that
// object is destroyed exactly once, whether it is a variable, an aggregate
// member, a by-value argument or a return value. kiln bound the conversion's
// result twice and destroyed it twice (a double free in a translated-string
// aggregate `{original, std::string{*this}}`; WP-811).
//@ expect: run
//@ stdout: 0 0 0 0 0 0 0 0 ctors=10 dtors=10
extern "C" int printf(const char*, ...);
int live = 0, ctors = 0, dtors = 0;
struct S {
  int id;
  S(const char*) : id(++ctors) { ++live; }
  S(const S&) : id(++ctors) { ++live; }
  S(S&&) : id(++ctors) { ++live; }
  ~S() { --live; ++dtors; }
};
struct Pair { S a, b; };
struct TL {
  const char* s;
  operator S() const { return s; }
  operator Pair() const { return {s, S{*this}}; }
};
void take(S) {}
S ret() { return S{TL{"r"}}; }
template <class T> T make() { return T{TL{"t"}}; }
int main() {
  { S x{TL{"a"}}; } printf("%d ", live);
  { S x = S{TL{"b"}}; } printf("%d ", live);
  { S x(TL{"c"}); } printf("%d ", live);
  { Pair p{"d", S{TL{"e"}}}; } printf("%d ", live);
  { take(S{TL{"f"}}); } printf("%d ", live);
  { S x = ret(); } printf("%d ", live);
  { Pair p = TL{"g"}; } printf("%d ", live);
  { S x = make<S>(); } printf("%d ", live);
  printf("ctors=%d dtors=%d\n", ctors, dtors);
}
