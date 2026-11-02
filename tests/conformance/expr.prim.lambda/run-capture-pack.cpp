// [expr.prim.lambda.capture]/17, [temp.variadic]/5.4: a simple-capture
// pack expansion `[ts...]` / `[&ts...]` in a function template captures
// each element of the instantiated pack; the body folds over the captured
// pack together with the lambda's own template parameter pack.
//@ expect: run
//@ stdout: 14 320
extern "C" int printf(const char *, ...);
template <unsigned long... I> struct seq {};
template <class... Ts> long byval(Ts... ts) {
  return [ts...]<unsigned long... P>(seq<P...>) { return ((ts * long(P + 1)) + ...); }(seq<0, 1, 2>());
}
template <class... Ts> long byref(Ts... ts) {
  return [&ts...]<class... Us>(Us... us) { return ((ts * us) + ... + 0); }(10, 100);
}
int main() { printf("%ld %ld\n", byval(1, 2, 3), byref(2, 3)); }
