namespace a { struct S { int x; }; namespace b { struct T { char c; S s; }; } }
namespace { struct U { a::b::T t; }; }
inline namespace v2 { struct V { char c; }; }
namespace n { inline namespace i { struct W { int w; }; } }
