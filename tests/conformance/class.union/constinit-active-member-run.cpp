// [basic.start.static]/2, [class.union.general]/2: a constant-initialized
// variable whose union member was made active by an assignment in a
// constructor body (or by a union's trivial copy assignment) is emitted
// with that member, which the evaluation recorded, not one named by an
// initializer list. spec/oven.md "Transient allocation" (WP-212).
//@ expect: run
//@ stdout: 3 7 9 5
extern "C" int printf(const char*, ...);

union U { char c; long l; };
struct A {
  U u;
  constexpr A() : u() { u.l = 3; }
};
struct B {
  U u;
  constexpr B() : u() { U t{}; t.l = 7; u = t; }
};
// Nested unions at one offset: the inner one's member is recorded too.
union Inner { short s; int i; };
union Outer { Inner in; long l; };
struct C {
  Outer o;
  constexpr C() : o() { Inner t{}; t.i = 9; o.in = t; }
};
constexpr U make(long v) { U r{}; r.l = v; return r; }

constinit A a;
constinit B b;
constinit C c;
constinit U d = make(5);

int main() {
  printf("%ld %ld %d %ld\n", a.u.l, b.u.l, c.o.in.i, d.l);
}
