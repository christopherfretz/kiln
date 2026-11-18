// Virtual calls through a base pointer, a pure virtual base and a virtual
// destructor.
#include <cstdio>
#include <memory>

struct Shape {
    virtual ~Shape() { std::puts("~Shape"); }
    virtual int area() const = 0;
    virtual const char* name() const { return "shape"; }
};

struct Square : Shape {
    int side;
    explicit Square(int s) : side(s) {}
    int area() const override { return side * side; }
    const char* name() const override { return "square"; }
};

struct Rect : Shape {
    int w, h;
    Rect(int a, int b) : w(a), h(b) {}
    int area() const override { return w * h; }
};

int main() {
    std::unique_ptr<Shape> shapes[] = {std::make_unique<Square>(3), std::make_unique<Rect>(2, 5)};
    int total = 0;
    for (auto& s : shapes) {
        std::printf("%s %d\n", s->name(), s->area());
        total += s->area();
    }
    return total == 19 ? 0 : 1;
}
