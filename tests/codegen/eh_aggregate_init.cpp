// [except.ctor]/3, [dcl.init.aggr]/7, [class.temporary]/5: when an
// element of a braced aggregate initialization throws, the bases,
// members and array elements already initialized are destroyed (the
// irregular partial-array cleanup and the deferred member cleanups),
// and the cleanups are deactivated once the object is complete.
int may_throw(int);
struct L { int id; L(int i); L(); ~L(); };
struct E { L arr[3]; E() : arr{L(1), L(2), L(-3)} {} };
void member_array() { E e; }
void local_array() { L a[3] = {L(1), L(2), L(3)}; }
void filler() { L a[5] = {L(1), L(2)}; }
void filler_only() { L a[4] = {}; }
void nested() { L a[2][2] = {{L(1), L(2)}, {L(3)}}; }
struct AG { L x; int n; L y; };
void members() { AG g = {L(1), may_throw(2), L(3)}; }
struct B1 { L l; };
struct B2 { int k; ~B2(); };
struct D : B1, B2 { L z; };
void bases() { D d = {{L(1)}, {may_throw(2)}, L(3)}; }
void array_of_aggregates() { AG a[2] = {{L(1), 1, L(2)}, {L(3), 2, L(4)}}; }
struct H { L arr[2]; int t; };
void member_array_filler() { H h = {{L(1)}, may_throw(5)}; }
L make(int);
void prvalues() { L a[2] = {make(1), make(2)}; }
