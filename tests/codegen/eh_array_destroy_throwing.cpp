// [except.ctor]/2: when a destructor that may throw ends the lifetime of
// an array element, the remaining elements are still destroyed: the
// destroy loop of a local array, of an array member in a destructor body
// and in a constructor's EH cleanup invokes the destructor under a
// partial-array EH cleanup (clang `useEHCleanupForArray`). A
// non-throwing destructor keeps plain calls.
struct S { int x; S(); ~S() noexcept(false); };
void use(S*);
void f() { S a[3]; use(a); }
struct H { S m[2]; H(); };
H::H() {}
void g() { H h; }
struct G { S m[2]; ~G(); };
G::~G() {}
struct N { int x; ~N(); };
void k() { N a[3]; }
