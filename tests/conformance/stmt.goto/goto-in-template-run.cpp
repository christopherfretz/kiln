// [temp.inst], [stmt.label], [stmt.goto]: labels, gotos and asm goto
// labels are instantiated by name; the instantiation's gotos run its
// cleanups (each loop iteration destroys d).
//@ expect: run
//@ stdout: ~D 0
//@ stdout: ~D 1
//@ stdout: ~D 2
//@ stdout: ~D 0
//@ stdout: ~D 1
//@ stdout: ~D 2
//@ stdout: ~D 3
//@ stdout: 3 6 0
extern "C" int printf(const char *, ...);
struct D { int n; ~D() { printf("~D %d\n", n); } };
template <class T> T sum(T n) {
  T s = 0, i = 0;
loop:
  if (i == n) goto done;
  {
    D d{(int)i};
    s += i++;
  }
  goto loop;
done:
  return s;
}
template <class T> int ag(T x) {
  asm goto("" : : "r"(x) : : out);
  return 0;
out:
  return 1;
}
int main() { printf("%d %ld %d\n", sum(3), sum(4L), ag(1)); }
