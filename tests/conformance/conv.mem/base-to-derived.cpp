// [conv.mem]/2: a pointer to member of a base converts to a pointer to member
// of a derived class and still designates the same member.
//@ expect: run
//@ stdout: 7 8 1
extern "C" int printf(const char*, ...);
struct B { int x = 7; };
struct M { int pad = 0; };
struct D : M, B { int y = 8; };
int main() {
  int D::*p = &B::x;
  int D::*q = &D::y;
  int D::*n = nullptr;
  D d;
  printf("%d %d %d\n", d.*p, d.*q, n == nullptr);
}
