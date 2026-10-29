// Class prvalues: direct member access on call results, temporaries
// passed straight through, and locals initialized from calls.
struct Big { long a, b, c, d, e; };
struct Small { int x, y; };
Big eb(int); Small es(int);
void sink(Big);
void sink_small(Small);
long temps(int x) {
  sink(eb(x));
  sink_small(es(x));
  Big b = eb(x);
  Small s = es(x);
  return eb(x).c + es(x).y + b.e + s.x;
}
