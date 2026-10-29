class P { int i; char c; };
struct Q { P p; char d; };
struct PodP { int i; char c; };
struct PodQ { [[no_unique_address]] PodP p; char d; };
