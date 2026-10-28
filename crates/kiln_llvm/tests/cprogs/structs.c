// Struct types, member access, struct copy (memcpy), sret and indirect args.
struct P { int x; long y; };
struct Big { long a, b, c; };
struct P g = {1, 2};
long sum(struct P *p) { return p->x + p->y; }
struct Big make(struct Big in) { struct Big r = in; r.a = 1; return r; }
