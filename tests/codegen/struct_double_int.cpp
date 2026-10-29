// { double, int } and { int, double }: an SSE and an INTEGER eightbyte.
struct DI { double d; int i; };
struct ID { int i; double d; };
DI edi(int); ID eid(int);
double tdi(DI v) { return v.d + v.i; }
double tid(ID v) { return v.d - v.i; }
DI pdi(int x) { return edi(x); }
double both(int x) { return tdi(edi(x)) * tid(eid(x)) + tdi(pdi(x)); }
