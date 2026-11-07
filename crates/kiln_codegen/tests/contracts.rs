//! Contract assertions in codegen (P2900; spec/contracts.md §6.3): the
//! callee-side checks, the constant `contract_violation` objects in
//! libstdc++'s layout and the weak default handler, as IR goldens (clang
//! implements no contracts, so there is no `ir` oracle).

use std::sync::Arc;

use kiln_abi::{Arch, Target};
use kiln_ast::stmt::ContractSemantic;
use kiln_codegen::{CodegenOptions, emit};
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// Lowers `src` under `semantic`: the IR text or the codegen error.
fn lower(src: &str, semantic: ContractSemantic) -> Result<String, String> {
    lower_with(src, semantic, true)
}

/// [`lower`], with or without kiln's default violation handler.
fn lower_with(src: &str, semantic: ContractSemantic, default: bool) -> Result<String, String> {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let target = Target::for_arch(Arch::AArch64);
    let opts = ParseOptions { contract_semantic: semantic, ..ParseOptions::default() };
    let mut tu =
        parse_tu(&sm, &interner, file, &engine, target.clone(), &opts, ConstexprLimits::default());
    let errors: Vec<String> =
        sink.take().into_iter().filter(|d| d.level >= Level::Error).map(|d| d.message).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let cg = CodegenOptions {
        source_filename: "t.cpp".to_owned(),
        contract_default_handler: default,
        ..CodegenOptions::default()
    };
    emit(&mut tu.ast, &mut tu.oven, &target, &cg).map(|m| m.to_string()).map_err(|e| e.msg)
}

const FUNCTION: &str = "int f(const int x) pre(x > 0) post(r: r > x) { return x + 1; }\n\
                        int g() { return f(1); }\n";
const DECLARED: &str = "int f(int x) pre(x > 0);\nint f(int x) { return x; }\n";
const ASSERT: &str = "int f(int x) { contract_assert(x > 0); return x; }\n";
const NONE: &str = "int f(int x) { return x; }\n";

#[test]
fn ignore_lowers_without_the_contracts() {
    for src in [FUNCTION, DECLARED, ASSERT] {
        let ir = lower(src, ContractSemantic::Ignore).expect("lowers under ignore");
        assert!(ir.contains("define"), "{ir}");
    }
}

const HANDLER: &str = "@_Z25handle_contract_violationRKNSt9contracts18contract_violationE";
const INVOKE_DEFAULT: &str =
    "@_ZNSt9contracts41invoke_default_contract_violation_handlerERKNS_18contract_violationE";
const THROWING: &str = "bool p(int);\nint f(int x) { contract_assert(p(x)); return x; }\n";

/// The text of the definition of `name` in `ir`.
fn body<'a>(ir: &'a str, name: &str) -> &'a str {
    let start = ir
        .match_indices("define ")
        .map(|(i, _)| i)
        .find(|&i| ir[i..].lines().next().is_some_and(|l| l.contains(&format!("@{name}("))))
        .unwrap_or_else(|| panic!("no definition of {name}: {ir}"));
    let end = ir[start..].find("\n}\n").map_or(ir.len(), |i| start + i + 3);
    &ir[start..end]
}

/// Asserts that `parts` occur in `text` in order.
fn in_order(text: &str, parts: &[&str]) {
    let mut rest = text;
    for p in parts {
        let i = rest.find(p).unwrap_or_else(|| panic!("missing {p:?} (in order) in:\n{text}"));
        rest = &rest[i + p.len()..];
    }
}

// [dcl.contract.func]/7, [basic.contract.eval]: under enforce the
// precondition runs after the parameters, the postcondition after the
// result is stored, each a handler call then std::terminate().
#[test]
fn enforce_golden() {
    let ir = lower(FUNCTION, ContractSemantic::Enforce).expect("lowers");
    in_order(
        &ir,
        &[
            "@.str = private unnamed_addr constant [6 x i8] c\"t.cpp\\00\", align 1\n",
            "@.str.1 = private unnamed_addr constant [17 x i8] c\"int f(const int)\\00\", align 1\n",
            "@.str.2 = private unnamed_addr constant [6 x i8] c\"x > 0\\00\", align 1\n",
            "@.contract.loc = private unnamed_addr constant { ptr, ptr, i32, i32 } \
             { ptr @.str, ptr @.str.1, i32 1, i32 20 }, align 8\n",
            "@.contract = private unnamed_addr constant { i16, i16, i16, i16, ptr, ptr, ptr } \
             { i16 1, i16 1, i16 3, i16 1, ptr @.str.2, ptr @.contract.loc, ptr null }, align 8\n",
            "@.contract.5 = private unnamed_addr constant { i16, i16, i16, i16, ptr, ptr, ptr } \
             { i16 1, i16 2, i16 3, i16 1, ptr @.str.3, ptr @.contract.loc.4, ptr null }, align 8\n",
        ],
    );
    let golden = "define dso_local noundef i32 @_Z1fi(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  store i32 %0, ptr %3, align 4
  %4 = load i32, ptr %3, align 4
  %5 = icmp sgt i32 %4, 0
  br i1 %5, label %7, label %6

6:
  call void @_Z25handle_contract_violationRKNSt9contracts18contract_violationE(ptr @.contract)
  call void @_ZSt9terminatev() #3
  unreachable

7:
  %8 = load i32, ptr %3, align 4
  %9 = add nsw i32 %8, 1
  store i32 %9, ptr %2, align 4
  %10 = load i32, ptr %2, align 4
  %11 = load i32, ptr %3, align 4
  %12 = icmp sgt i32 %10, %11
  br i1 %12, label %14, label %13

13:
  call void @_Z25handle_contract_violationRKNSt9contracts18contract_violationE(ptr @.contract.5)
  call void @_ZSt9terminatev() #3
  unreachable

14:
  %15 = load i32, ptr %2, align 4
  ret i32 %15
}
";
    assert_eq!(body(&ir, "_Z1fi"), golden);
    // The default handler: weak, so a user's definition replaces it.
    assert!(ir.contains(&format!("define weak void {HANDLER}(ptr noundef %0) #1 {{")), "{ir}");
    in_order(
        body(&ir, &INVOKE_DEFAULT[1..]),
        &[
            "define weak void",
            "load ptr, ptr @stdout, align 8",
            "call i32 @fflush(",
            "load ptr, ptr @stderr, align 8",
            "call i32 (ptr, ptr, ...) @fprintf(",
            "ret void",
        ],
    );
    assert!(ir.contains("c\"contract violation in function %s at %s:%u: %s\\0A[assertion_kind: %s, \
                         semantic: %s, mode: %s, terminating: %s]\\0A\\00\""));
}

// [basic.contract.eval]/7: an exception escaping the predicate is a
// violation detected as evaluation_exception, whose handler runs inside
// the implicit `catch (...)`; observe then continues.
#[test]
fn observe_detection_modes() {
    let ir = lower(THROWING, ContractSemantic::Observe).expect("lowers");
    in_order(
        &ir,
        &[
            "@.contract = private unnamed_addr constant { i16, i16, i16, i16, ptr, ptr, ptr } \
             { i16 1, i16 3, i16 2, i16 2,",
            "@.contract.4 = private unnamed_addr constant { i16, i16, i16, i16, ptr, ptr, ptr } \
             { i16 1, i16 3, i16 2, i16 1,",
        ],
    );
    in_order(
        body(&ir, "_Z1fi"),
        &[
            "invoke noundef i1 @_Z1pi(",
            "landingpad { ptr, i32 }\n          catch ptr null",
            "call ptr @__cxa_begin_catch(",
            &format!("invoke void {HANDLER}(ptr @.contract)"),
            "call void @__cxa_end_catch()",
            &format!("call void {HANDLER}(ptr @.contract.4)\n  br label"),
            "ret i32",
        ],
    );
    assert!(!body(&ir, "_Z1fi").contains("@_ZSt9terminatev"), "{ir}");
}

// [basic.contract.eval]/11: quick_enforce terminates without the handler;
// an escaping exception reaches std::terminate() too.
#[test]
fn quick_enforce_terminates() {
    let ir = lower(THROWING, ContractSemantic::QuickEnforce).expect("lowers");
    let f = body(&ir, "_Z1fi");
    in_order(f, &["invoke noundef i1 @_Z1pi(", "call void @_ZSt9terminatev()", "unreachable"]);
    assert!(f.contains("@__clang_call_terminate"), "{f}");
    assert!(!ir.contains(HANDLER) && !ir.contains("@.contract"), "{ir}");
}

// spec/contracts.md §6.3: with a standard library that ships its own
// handler (libstdc++ 16) codegen only declares it.
#[test]
fn no_default_handler_with_the_stdlib_one() {
    let ir = lower_with(FUNCTION, ContractSemantic::Enforce, false).expect("lowers");
    assert!(ir.contains(&format!("declare void {HANDLER}(ptr)")), "{ir}");
    assert!(!ir.contains(INVOKE_DEFAULT) && !ir.contains("@fprintf"), "{ir}");
}

// A function without contracts pays nothing, and declarations alone
// check nothing at the definition's callers.
#[test]
fn checking_semantics_cost_nothing_without_contracts() {
    for k in [ContractSemantic::Observe, ContractSemantic::Enforce, ContractSemantic::QuickEnforce]
    {
        let plain = lower(NONE, k).expect("lowers");
        assert_eq!(plain, lower(NONE, ContractSemantic::Ignore).expect("lowers"), "{k:?}");
        assert!(!plain.contains("contract") && !plain.contains("terminate"), "{plain}");
        for src in [FUNCTION, DECLARED, ASSERT] {
            let ir = lower(src, k).expect("a checked contract lowers");
            assert!(ir.contains("@_ZSt9terminatev") || k == ContractSemantic::Observe, "{ir}");
        }
    }
}

/// A minimal coroutine library: just what a `co_return` needs.
const CORO: &str = "\
namespace std {\n\
template <class R, class...> struct coroutine_traits { using promise_type = typename R::promise_type; };\n\
template <class P = void> struct coroutine_handle {\n\
  void* p;\n\
  static coroutine_handle from_address(void* a) noexcept { return {a}; }\n\
  void* address() const noexcept { return p; }\n\
};\n\
struct suspend_never {\n\
  bool await_ready() const noexcept { return true; }\n\
  template <class H> void await_suspend(H) const noexcept {}\n\
  void await_resume() const noexcept {}\n\
};\n\
}\n\
struct task {\n\
  struct promise_type {\n\
    task get_return_object() { return {}; }\n\
    std::suspend_never initial_suspend() noexcept { return {}; }\n\
    std::suspend_never final_suspend() noexcept { return {}; }\n\
    void return_void() {}\n\
    void unhandled_exception() {}\n\
  };\n\
};\n\
";

// [dcl.contract.func], [dcl.fct.def.coroutine]: a coroutine's
// precondition runs at entry, before the frame is set up; its
// postcondition is unsupported in codegen (spec/contracts.md §9) and is a
// clean error rather than a silently skipped check.
#[test]
fn coroutine_precondition_and_unsupported_postcondition() {
    let pre = format!("{CORO}task c(const int& x) pre(x > 0) {{ co_return; }}\n");
    let ir = lower(&pre, ContractSemantic::Enforce).expect("a coroutine precondition lowers");
    assert!(body(&ir, "_Z1cRKi").contains(&format!("call void {HANDLER}(")), "{ir}");
    let post = format!("{CORO}task c(const int& x) post(x > 1) {{ co_return; }}\n");
    let err = lower(&post, ContractSemantic::Enforce).expect_err("unsupported");
    assert!(err.contains("unsupported: postcondition of a coroutine"), "{err}");
    lower(&post, ContractSemantic::Ignore).expect("nothing to check under ignore");
}
