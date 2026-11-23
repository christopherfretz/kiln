// [cpp.stringize]/2: `#__VA_ARGS__` stringizes the variadic argument, one
// argument with its commas; an empty or omitted one gives `""`. The made
// row names the argument by index (spec/pp.md "Preprocessing record",
// made tokens).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: va-args-stringize.record
#define S(...) #__VA_ARGS__
#define S2(x, ...) #__VA_ARGS__
const char *a = S(a, b);
const char *b = S();
const char *c = S2(1);
