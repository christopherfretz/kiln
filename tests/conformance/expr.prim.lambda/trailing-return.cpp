// [expr.prim.lambda.closure]/4: the return type is the trailing return type
// if present, otherwise deduced from the return statements.
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
static_assert(same<decltype([] { return 1; }()), int>);
static_assert(same<decltype([] -> long { return 1; }()), long>);
static_assert(same<decltype([] {}()), void>);
static_assert(same<decltype([](int& r) -> int& { return r; }(*(int*)nullptr)), int&>);
static_assert(same<decltype([] -> decltype(auto) { return 1.0; }()), double>);
static_assert(same<decltype([](char c) { return c; }('a')), char>);
