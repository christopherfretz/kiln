// [class.bit], [basic.start.static]: an object with bit-fields that is
// constant-initialized is emitted from its constant value (the storage
// units hold the packed fields), including inside a union's active member
// and in a packed anonymous struct, as libc++'s short string lays them out.
//@ expect: run
struct B { unsigned a : 3; unsigned b : 5; int c : 7; unsigned long d : 40; char e; };
constexpr B gb{5, 17, -9, 0x12345678ffull, 'x'};
struct P { struct __attribute__((packed)) { unsigned char l : 1; unsigned char s : 7; }; char pad; char data[22]; };
struct L { unsigned long cap : 63; unsigned long is_long : 1; unsigned long size; char *data; };
union U { L l; P s; };
constexpr U gu{L{12345, 1, 0, nullptr}};
constinit U gz{};
int main() {
  const B &r = gb;
  if (r.a != 5 || r.b != 17 || r.c != -9 || r.d != 0x12345678ffull || r.e != 'x') return 1;
  const U &u = gu;
  if (u.l.cap != 12345 || u.l.is_long != 1 || u.l.size != 0) return 2;
  if (gz.l.cap != 0 || gz.l.is_long != 0) return 3;
  U lu{};
  lu.s = P{{1, 42}, 0, {'h', 'i'}};
  if (lu.s.l != 1 || lu.s.s != 42 || lu.s.data[1] != 'i') return 5;
  B lb{1, 2, -3, 4, 'y'};
  if (lb.c != -3 || lb.d != 4) return 4;
  return 0;
}
