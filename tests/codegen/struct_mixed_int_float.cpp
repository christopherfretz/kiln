// Mixed int/float members: an eightbyte is INTEGER when any part is.
struct IF { int i; float f; };
struct FI { float f; int i; };
struct DL { double d; long l; };
struct LD { long l; double d; };
IF eif(int); FI efi(int); DL edl(int); LD eld(int);
float tif(IF v) { return v.i + v.f; }
float tfi(FI v) { return v.f * v.i; }
double tdl(DL v) { return v.d + v.l; }
double tld(LD v) { return v.d - v.l; }
IF pif(int x) { return eif(x); }
DL pdl(int x) { return edl(x); }
double all(int x) { return tif(eif(x)) + tfi(efi(x)) + tdl(edl(x)) + tld(eld(x)) + tif(pif(x)) + tdl(pdl(x)); }
