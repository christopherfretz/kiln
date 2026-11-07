// [basic.contract.eval]/1 (P2900R14): under the ignore evaluation semantic
// (-fcontract-evaluation-semantic=ignore, GCC's spelling) no predicate is
// evaluated, so a violated precondition, postcondition or contract_assert
// neither diagnoses nor terminates, and the program runs as without them.
// GCC 16.1 prints the same.
//@ expect: run
//@ args: -fcontract-evaluation-semantic=ignore
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ stdout: 3 7 9
extern "C" int printf(const char*, ...);
int calls;
int f(int x) pre(x > 100) post(r: r < 0) { ++calls; return x + 2; }
struct S {
  int m;
  int add(int a) const pre(a > m) { contract_assert(a < 0); return a + m; }
};
template <class T> T tw(T t) pre(t == T{}) { return t * 3; }
auto lam = [](int a) post(r: r == 0) { return a; };
int main() {
  int a = f(1);
  int b = S{3}.add(4);
  int c = tw(3);
  printf("%d %d %d\n", a, b, c);
  return lam(0) + calls - 1;
}
