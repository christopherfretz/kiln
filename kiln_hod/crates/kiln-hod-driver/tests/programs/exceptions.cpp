// A throw caught two frames up, a rethrow, and a destructor run while
// unwinding.
#include <cstdio>
#include <stdexcept>

struct Noisy {
    const char* name;
    ~Noisy() { std::printf("~%s\n", name); }
};

static int depth(int n) {
    Noisy guard{"guard"};
    if (n == 0) throw std::runtime_error("bottom");
    return depth(n - 1) + 1;
}

int main() {
    try {
        try {
            depth(2);
        } catch (const std::exception& e) {
            std::printf("caught %s\n", e.what());
            throw;
        }
    } catch (const std::runtime_error& e) {
        std::printf("rethrown %s\n", e.what());
    }
    return 3;
}
