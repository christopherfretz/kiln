// [temp.inst]/11, GNU `used`: the `used` member functions of an
// implicitly instantiated class template specialization (and of an
// instantiated member class) are defined although nothing in the
// translation unit calls them: a call through a declaration naming the
// symbol with an asm label links and runs. Found in downstream code
// (debug-dump members of class templates).
//@ expect: run
//@ stdout: 10 7 9
extern "C" int printf(const char *, ...);
template <class T> struct S {
  int v = 5;
  __attribute__((used)) int dump() const { return v * 2; }
  __attribute__((used)) static int sm() { return 7; }
  struct N { [[gnu::used]] static int nf() { return 9; } };
};
S<int> s;
S<int>::N n;
int call_dump(const S<int> *) asm("_ZNK1SIiE4dumpEv");
int call_sm() asm("_ZN1SIiE2smEv");
int call_nf() asm("_ZN1SIiE1N2nfEv");
int main() { printf("%d %d %d\n", call_dump(&s), call_sm(), call_nf()); }
