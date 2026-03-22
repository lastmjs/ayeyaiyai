use std::collections::HashSet;

use crate::frontend;

use super::{
    DirectWasmCompiler, Expression, collect_eval_local_function_declarations,
    object_binding_lookup_value,
};

#[test]
fn collects_eval_local_function_declarations_in_ordinary_eval_context() {
    let compiler = DirectWasmCompiler::default();
    let program = compiler
        .parse_eval_program_in_ordinary_function_context_static(
            "initial = f; function f() { return 33; }",
        )
        .expect("ordinary eval wrapper should parse");

    let local_function_names = program
        .functions
        .iter()
        .filter(|function| !function.register_global)
        .map(|function| function.name.clone())
        .collect::<HashSet<_>>();
    let declarations =
        collect_eval_local_function_declarations(&program.statements, &local_function_names);

    assert_eq!(declarations.len(), 1);
    let binding_name = declarations
        .keys()
        .next()
        .expect("expected one eval-local function declaration binding");
    assert!(binding_name.starts_with("__ayy_scope$f$"));
}

#[test]
fn collects_parameter_object_bindings_for_repeated_getter_spread_calls() {
    let program = frontend::parse(
        r#"
            let getterCallCount = 0;
            let repeated = {
              get a() {
                return ++getterCallCount;
              }
            };
            (function(second) {
              console.log(second.a, second.c, second.d, Object.keys(second).length);
            })({ ...repeated, c: 4, d: 5, a: 42, ...repeated });
            "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    assert!(
        compiler
            .infer_global_member_getter_binding(
                &Expression::Identifier("repeated".to_string()),
                &Expression::String("a".to_string())
            )
            .is_some(),
        "expected repeated.a getter binding"
    );
    assert_eq!(
        compiler.infer_global_member_getter_return_value(
            &Expression::Identifier("repeated".to_string()),
            &Expression::String("a".to_string())
        ),
        Some(Expression::Number(1.0))
    );

    let repeated_copy = compiler
        .infer_global_copy_data_properties_binding(&Expression::Identifier("repeated".to_string()))
        .expect("expected repeated copy binding");
    assert_eq!(
        object_binding_lookup_value(&repeated_copy, &Expression::String("a".to_string())),
        Some(&Expression::Number(1.0))
    );

    let call_argument = match &program.statements[2] {
        crate::ir::hir::Statement::Expression(Expression::Call { arguments, .. }) => {
            match arguments.first().expect("expected callback call argument") {
                crate::ir::hir::CallArgument::Expression(expression)
                | crate::ir::hir::CallArgument::Spread(expression) => expression,
            }
        }
        _ => panic!("expected top-level callback call expression"),
    };
    let object_argument_binding = compiler
        .infer_global_object_binding(call_argument)
        .expect("expected object literal binding");
    assert_eq!(
        object_binding_lookup_value(
            &object_argument_binding,
            &Expression::String("a".to_string())
        ),
        Some(&Expression::Number(2.0))
    );

    let (_, _, _, object_bindings) = compiler.collect_user_function_parameter_bindings(&program);

    let function_name = program
        .functions
        .iter()
        .find(|function| {
            function
                .params
                .iter()
                .map(|param| param.name.as_str())
                .eq(["second"])
        })
        .expect("expected anonymous callback function")
        .name
        .clone();
    let binding = object_bindings
        .get(&function_name)
        .and_then(|bindings| bindings.get("second"))
        .cloned()
        .flatten()
        .expect("expected second parameter object binding");

    assert_eq!(
        object_binding_lookup_value(&binding, &Expression::String("a".to_string())),
        Some(&Expression::Number(2.0))
    );
    assert_eq!(
        object_binding_lookup_value(&binding, &Expression::String("c".to_string())),
        Some(&Expression::Number(4.0))
    );
    assert_eq!(
        object_binding_lookup_value(&binding, &Expression::String("d".to_string())),
        Some(&Expression::Number(5.0))
    );
}

#[test]
fn collects_parameter_object_bindings_for_symbol_spread_calls() {
    let program = frontend::parse(
        r#"
            let symbol = Symbol('foo');
            let o = {};
            o[symbol] = 1;
            (function(obj) {
              console.log(obj[symbol], obj.c, obj.d);
            }.apply(null, [{...o, c: 4, d: 5}]));
            "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let (_, _, _, object_bindings) = compiler.collect_user_function_parameter_bindings(&program);

    let function_name = program
        .functions
        .iter()
        .find(|function| {
            function
                .params
                .iter()
                .map(|param| param.name.as_str())
                .eq(["obj"])
        })
        .expect("expected anonymous callback function")
        .name
        .clone();
    let binding = object_bindings
        .get(&function_name)
        .and_then(|bindings| bindings.get("obj"))
        .cloned()
        .flatten()
        .expect("expected obj parameter object binding");

    assert_eq!(
        object_binding_lookup_value(&binding, &Expression::Identifier("symbol".to_string())),
        Some(&Expression::Number(1.0))
    );
    assert_eq!(
        object_binding_lookup_value(&binding, &Expression::String("c".to_string())),
        Some(&Expression::Number(4.0))
    );
    assert_eq!(
        object_binding_lookup_value(&binding, &Expression::String("d".to_string())),
        Some(&Expression::Number(5.0))
    );
}
