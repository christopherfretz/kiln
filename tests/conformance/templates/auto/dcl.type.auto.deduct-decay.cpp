// [temp.deduct.call]/2: a non-reference placeholder decays arrays and
// functions; a reference placeholder binds to them.
//@ strict-diags
int arr[3];
const char str[] = "ab";
int fn(int);
auto a = arr;
auto b = str;
auto c = fn;
auto &d = arr;
auto &e = fn;
auto f = "lit";
auto &g = "lit";
static_assert(__is_same(decltype(a), int *));
static_assert(__is_same(decltype(b), const char *));
static_assert(__is_same(decltype(c), int (*)(int)));
static_assert(__is_same(decltype(d), int (&)[3]));
static_assert(__is_same(decltype(e), int (&)(int)));
static_assert(__is_same(decltype(f), const char *));
static_assert(__is_same(decltype(g), const char (&)[4]));
