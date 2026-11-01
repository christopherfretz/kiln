// [basic.start.dynamic], [except.ctor]: an initializer that may throw is
// invoked with a landing pad that destroys its full-expression
// temporaries; the global's destructor is registered only after its
// initialization completes.
struct A { A(int); A(const A &); ~A(); int x; };
int take(A);
A make(const A &);
A g1 = make(A(1));
int g2 = take(A(2)) + take(3);
const A &g3 = make(4);
struct P { int a, b; };
P gp = {take(5), 6};
int arr[3] = {1, take(7), 3};
int *pp = &arr[1];
int (*fp)(A) = take;
inline int lam = [] { return 4; }();
int sel = sizeof(take(1));
int use() { return lam; }
