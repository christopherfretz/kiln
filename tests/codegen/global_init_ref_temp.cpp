// [class.temporary]/6, [basic.start.static]: a temporary bound to a
// reference with static storage duration is a _ZGR global, holding its
// constant value when the reference is constant-initialized and
// otherwise initialized (and its destructor registered) by the
// reference's initialization function.
struct S { int x; };
struct T { int x; T(int); ~T(); };
int f();
const int &a = 5;
const S &b = S{1};
int &&c = 5;
const int &d = f();
const T &e = T(2);
const T &e2 = 3;
inline const int &ia = f();
struct Q { static const int &qm; };
const int &Q::qm = 8;
const int *use() { return &ia; }
