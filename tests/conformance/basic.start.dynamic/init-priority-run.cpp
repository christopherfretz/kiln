// [basic.start.dynamic]/1,2 with GNU `init_priority(N)` (WP-189): the
// relative order of the dynamic initializations of namespace-scope objects
// is ordered by definition within a TU; `init_priority` is the
// implementation-defined extension that runs a prioritized object before
// every unprioritized one, lower priorities first and equal priorities in
// definition order. Destruction is in reverse construction order
// ([basic.start.term]/3).
//@ expect: run
//@ stdout: P101 P200a P200b P300 P65535 U1 U2 main ~U2 ~U1 ~P65535 ~P300 ~P200b ~P200a ~P101
extern "C" int printf(const char*, ...);

struct A {
  const char *n;
  A(const char *s) : n(s) { printf("%s ", n); }
  ~A() { printf(n[1] == '1' && n[2] == '0' ? "~%s\n" : "~%s ", n); }
};
A u1("U1");
A p200a __attribute__((init_priority(200))) ("P200a");
A p65535 __attribute__((init_priority(65535))) ("P65535");
[[gnu::init_priority(300)]] A p300("P300");
A u2("U2");
A p101 __attribute__((init_priority(101))) ("P101");
A p200b __attribute__((init_priority(200))) ("P200b");

int main() { printf("main "); }
