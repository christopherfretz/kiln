// Dynamic initialization of namespace-scope variables in definition order
// (an ordered initialization within one translation unit), a function-local
// static, and destruction in reverse order.
#include <cstdio>

static int step = 0;

struct Logged {
    const char* name;
    int at;
    explicit Logged(const char* n) : name(n), at(++step) { std::printf("init %s %d\n", n, at); }
    ~Logged() { std::printf("fini %s\n", name); }
};

Logged first("first");
Logged second("second");

static Logged& local() {
    static Logged l("local");
    return l;
}

int third_value = second.at + 1;

int main() {
    std::printf("main %d %d\n", local().at, third_value);
    return 0;
}
