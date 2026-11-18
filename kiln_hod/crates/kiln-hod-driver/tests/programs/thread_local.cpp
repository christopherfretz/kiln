// A thread_local with a dynamic initializer and a destructor, read from
// two threads (POSIX threads: <thread> brings always_inline functions,
// which the in-process route leaves to the LLVM route).
#include <cstdio>
#include <pthread.h>

static int made = 0;

struct Counter {
    int value;
    Counter() : value(++made * 10) {}
    ~Counter() {}
};

thread_local Counter counter;

static int bump() { return ++counter.value; }

static void* worker(void* out) {
    *static_cast<int*>(out) = bump();
    return nullptr;
}

int main() {
    int a = bump();
    int b = 0;
    pthread_t t;
    if (pthread_create(&t, nullptr, worker, &b) != 0) return 2;
    pthread_join(t, nullptr);
    int c = bump();
    std::printf("%d %d %d made=%d\n", a, b, c, made);
    return 0;
}
