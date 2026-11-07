// [class.union.anon]/1, [class.mem.general]: an unnamed class defined in
// the type-id of an alias-declaration (`using I = struct {...};`) is not
// an anonymous struct or union, even though its `}` is followed by `;`.
// A member of that type keeps its default member initializer, so the
// constructor value-initializes it. Reduced from LLVM's MachObjectWriter
// (`VersionInfoType VersionInfo{};`), which read garbage otherwise.
//@ expect: run
//@ stdout: 0 0 0 0 7 7
#include <cstdio>
#include <cstring>
#include <new>

using I = struct { unsigned a, b; };
using U = union { unsigned u; float f; };
typedef struct { int x; } T;

struct W {
  I i{};
  U u{};
  T t{7};
  I j = {7, 7};
  W() {}
};

int main() {
  alignas(W) unsigned char buf[sizeof(W)];
  std::memset(buf, 0xAB, sizeof buf);
  W *w = new (buf) W;
  std::printf("%u %u %u %d %u %u\n", w->i.a, w->i.b, w->u.u, w->t.x - 7, w->j.a,
              w->j.b);
  w->~W();
}
