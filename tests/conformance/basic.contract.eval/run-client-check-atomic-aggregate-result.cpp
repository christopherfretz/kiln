// -fcontracts-client-check=all with -fcontracts-definition-check=off
// (spec/contracts.md §6.1): a caller-side postcondition that names the
// result of a function returning an _Atomic class type (returned in
// memory, an aggregate) checks the returned object. kiln's call emitted
// no result object to check ("a caller-side postcondition's result ...
// is not supported yet") when the call's value was discarded or used as
// an operand.
//@ expect: run
//@ args: -fcontract-evaluation-semantic=enforce -fcontracts-client-check=all -fcontracts-definition-check=off
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: none
//@ stdout: post 8
//@ stdout: post 24
//@ stdout: post 8
//@ stdout: done
extern "C" int printf(const char *, ...);
bool seen(unsigned long n) {
  printf("post %lu\n", n);
  return true;
}
struct S {
  int a, b;
};
_Atomic(S) make(int a) post(r: seen(sizeof(r))) { return S{a, a + 1}; }
struct L {
  long a, b, c;
};
_Atomic(L) big() post(r: seen(sizeof(r))) { return L{7, 8, 9}; }

int main() {
  make(1);
  big();
  _Atomic(S) s = make(3);
  (void)s;
  printf("done\n");
}
