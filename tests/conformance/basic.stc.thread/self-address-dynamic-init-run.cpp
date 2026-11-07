// [expr.const]/14, [basic.start.static]/2, [basic.stc.thread]/1: the
// address of an object with thread storage duration is not a permitted
// result of a constant expression, so a `thread_local` variable whose
// initializer stores its own address (`this` in a default member
// initializer, a small-buffer pointer into itself) is not
// constant-initialized. It is dynamically initialized for each thread,
// and the stored address is that thread's object, not an address frozen
// into the `.tdata` image.
//@ expect: run
//@ strict-diags
//@ stdout: 1 1 1 | 1 1 1 | 3 ok
extern "C" int printf(const char*, ...);

struct S { S* self = this; int v = 1; };
struct Buf {
  char small[8] = {'o', 'k', 0};
  char* p = small;
};

thread_local S g;
S& s() { static thread_local S x; return x; }
Buf& buf() { thread_local Buf b; return b; }

int main() {
  printf("%d %d %d | ", g.self == &g, s().self == &s(), buf().p == buf().small);
  printf("%d %d %d | ", g.v, s().v, buf().p[0] == 'o');
  ++g.self->v;
  ++s().self->v;
  printf("%d %s\n", g.v + s().v - 1, buf().p);
}
