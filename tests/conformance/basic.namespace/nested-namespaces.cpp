//@ expect: compile-ok
namespace outer {
int v = 1;
namespace inner {
int w = v + 2;
}
inline namespace ver {
int x;
}
}
namespace outer::deep {
int y = inner::w;
}
int z = outer::deep::y + outer::x;
