// [class.temporary]/2, [dcl.init.general]/17.6.1 (C++17, P0135R1):
// guaranteed copy elision; a prvalue initializes its result object
// directly, so non-copyable, non-movable types can be returned and passed.
//@ expect: run
//@ std: c++17
//@ strict-diags
//@ stdout: 1 7 9 1
extern "C" int printf(const char*, ...);
struct Pin {
  int v;
  Pin* self;
  explicit Pin(int x) : v(x), self(this) {}
  Pin(const Pin&) = delete;
  Pin(Pin&&) = delete;
};
Pin make(int x) { return Pin(x); }
Pin twice(int x) { return make(x); }
int take(Pin p) { return p.v; }
struct Holder { Pin p; };
int main() {
  Pin a = make(1);
  Pin b = Pin(Pin(Pin(7)));
  Holder h{twice(9)};
  printf("%d %d %d %d\n", a.v, take(Pin(b.v)), h.p.v,
         a.self == &a && h.p.self == &h.p);
}
