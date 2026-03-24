use crate::ir::hir::{
    ArrayElement, CallArgument, Expression, FunctionDeclaration, ObjectEntry, Parameter, Program,
    Statement, SwitchCase,
};

pub trait Visitor {
    fn visit_program(&mut self, program: &Program) {
        walk_program(self, program);
    }

    fn visit_function_declaration(&mut self, function: &FunctionDeclaration) {
        walk_function_declaration(self, function);
    }

    fn visit_parameter(&mut self, parameter: &Parameter) {
        walk_parameter(self, parameter);
    }

    fn visit_statement(&mut self, statement: &Statement) {
        walk_statement(self, statement);
    }

    fn visit_switch_case(&mut self, switch_case: &SwitchCase) {
        walk_switch_case(self, switch_case);
    }

    fn visit_expression(&mut self, expression: &Expression) {
        walk_expression(self, expression);
    }

    fn visit_array_element(&mut self, element: &ArrayElement) {
        walk_array_element(self, element);
    }

    fn visit_object_entry(&mut self, entry: &ObjectEntry) {
        walk_object_entry(self, entry);
    }

    fn visit_call_argument(&mut self, argument: &CallArgument) {
        walk_call_argument(self, argument);
    }
}

pub trait VisitorMut {
    fn visit_program_mut(&mut self, program: &mut Program) {
        walk_program_mut(self, program);
    }

    fn visit_function_declaration_mut(&mut self, function: &mut FunctionDeclaration) {
        walk_function_declaration_mut(self, function);
    }

    fn visit_parameter_mut(&mut self, parameter: &mut Parameter) {
        walk_parameter_mut(self, parameter);
    }

    fn visit_statement_mut(&mut self, statement: &mut Statement) {
        walk_statement_mut(self, statement);
    }

    fn visit_switch_case_mut(&mut self, switch_case: &mut SwitchCase) {
        walk_switch_case_mut(self, switch_case);
    }

    fn visit_expression_mut(&mut self, expression: &mut Expression) {
        walk_expression_mut(self, expression);
    }

    fn visit_array_element_mut(&mut self, element: &mut ArrayElement) {
        walk_array_element_mut(self, element);
    }

    fn visit_object_entry_mut(&mut self, entry: &mut ObjectEntry) {
        walk_object_entry_mut(self, entry);
    }

    fn visit_call_argument_mut(&mut self, argument: &mut CallArgument) {
        walk_call_argument_mut(self, argument);
    }
}

pub fn walk_program<V: Visitor + ?Sized>(visitor: &mut V, program: &Program) {
    for function in &program.functions {
        visitor.visit_function_declaration(function);
    }
    for statement in &program.statements {
        visitor.visit_statement(statement);
    }
}

pub fn walk_program_mut<V: VisitorMut + ?Sized>(visitor: &mut V, program: &mut Program) {
    for function in &mut program.functions {
        visitor.visit_function_declaration_mut(function);
    }
    for statement in &mut program.statements {
        visitor.visit_statement_mut(statement);
    }
}

pub fn walk_function_declaration<V: Visitor + ?Sized>(
    visitor: &mut V,
    function: &FunctionDeclaration,
) {
    for parameter in &function.params {
        visitor.visit_parameter(parameter);
    }
    for statement in &function.body {
        visitor.visit_statement(statement);
    }
}

pub fn walk_function_declaration_mut<V: VisitorMut + ?Sized>(
    visitor: &mut V,
    function: &mut FunctionDeclaration,
) {
    for parameter in &mut function.params {
        visitor.visit_parameter_mut(parameter);
    }
    for statement in &mut function.body {
        visitor.visit_statement_mut(statement);
    }
}

pub fn walk_parameter<V: Visitor + ?Sized>(visitor: &mut V, parameter: &Parameter) {
    if let Some(default) = &parameter.default {
        visitor.visit_expression(default);
    }
}

pub fn walk_parameter_mut<V: VisitorMut + ?Sized>(visitor: &mut V, parameter: &mut Parameter) {
    if let Some(default) = &mut parameter.default {
        visitor.visit_expression_mut(default);
    }
}

pub fn walk_statement<V: Visitor + ?Sized>(visitor: &mut V, statement: &Statement) {
    match statement {
        Statement::Block { body } | Statement::Labeled { body, .. } => {
            for statement in body {
                visitor.visit_statement(statement);
            }
        }
        Statement::Var { value, .. }
        | Statement::Let { value, .. }
        | Statement::Assign { value, .. }
        | Statement::Expression(value)
        | Statement::Throw(value)
        | Statement::Return(value)
        | Statement::Yield { value }
        | Statement::YieldDelegate { value } => visitor.visit_expression(value),
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            visitor.visit_expression(object);
            visitor.visit_expression(property);
            visitor.visit_expression(value);
        }
        Statement::Print { values } => {
            for value in values {
                visitor.visit_expression(value);
            }
        }
        Statement::Break { .. } | Statement::Continue { .. } => {}
        Statement::With { object, body } => {
            visitor.visit_expression(object);
            for statement in body {
                visitor.visit_statement(statement);
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visitor.visit_expression(condition);
            for statement in then_branch {
                visitor.visit_statement(statement);
            }
            for statement in else_branch {
                visitor.visit_statement(statement);
            }
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            for statement in body {
                visitor.visit_statement(statement);
            }
            for statement in catch_setup {
                visitor.visit_statement(statement);
            }
            for statement in catch_body {
                visitor.visit_statement(statement);
            }
        }
        Statement::Switch {
            discriminant,
            cases,
            ..
        } => {
            visitor.visit_expression(discriminant);
            for switch_case in cases {
                visitor.visit_switch_case(switch_case);
            }
        }
        Statement::For {
            init,
            condition,
            update,
            break_hook,
            body,
            ..
        } => {
            for statement in init {
                visitor.visit_statement(statement);
            }
            if let Some(condition) = condition {
                visitor.visit_expression(condition);
            }
            if let Some(update) = update {
                visitor.visit_expression(update);
            }
            if let Some(break_hook) = break_hook {
                visitor.visit_expression(break_hook);
            }
            for statement in body {
                visitor.visit_statement(statement);
            }
        }
        Statement::While {
            condition,
            break_hook,
            body,
            ..
        }
        | Statement::DoWhile {
            condition,
            break_hook,
            body,
            ..
        } => {
            visitor.visit_expression(condition);
            if let Some(break_hook) = break_hook {
                visitor.visit_expression(break_hook);
            }
            for statement in body {
                visitor.visit_statement(statement);
            }
        }
    }
}

pub fn walk_statement_mut<V: VisitorMut + ?Sized>(visitor: &mut V, statement: &mut Statement) {
    match statement {
        Statement::Block { body } | Statement::Labeled { body, .. } => {
            for statement in body {
                visitor.visit_statement_mut(statement);
            }
        }
        Statement::Var { value, .. }
        | Statement::Let { value, .. }
        | Statement::Assign { value, .. }
        | Statement::Expression(value)
        | Statement::Throw(value)
        | Statement::Return(value)
        | Statement::Yield { value }
        | Statement::YieldDelegate { value } => visitor.visit_expression_mut(value),
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            visitor.visit_expression_mut(object);
            visitor.visit_expression_mut(property);
            visitor.visit_expression_mut(value);
        }
        Statement::Print { values } => {
            for value in values {
                visitor.visit_expression_mut(value);
            }
        }
        Statement::Break { .. } | Statement::Continue { .. } => {}
        Statement::With { object, body } => {
            visitor.visit_expression_mut(object);
            for statement in body {
                visitor.visit_statement_mut(statement);
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visitor.visit_expression_mut(condition);
            for statement in then_branch {
                visitor.visit_statement_mut(statement);
            }
            for statement in else_branch {
                visitor.visit_statement_mut(statement);
            }
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            for statement in body {
                visitor.visit_statement_mut(statement);
            }
            for statement in catch_setup {
                visitor.visit_statement_mut(statement);
            }
            for statement in catch_body {
                visitor.visit_statement_mut(statement);
            }
        }
        Statement::Switch {
            discriminant,
            cases,
            ..
        } => {
            visitor.visit_expression_mut(discriminant);
            for switch_case in cases {
                visitor.visit_switch_case_mut(switch_case);
            }
        }
        Statement::For {
            init,
            condition,
            update,
            break_hook,
            body,
            ..
        } => {
            for statement in init {
                visitor.visit_statement_mut(statement);
            }
            if let Some(condition) = condition {
                visitor.visit_expression_mut(condition);
            }
            if let Some(update) = update {
                visitor.visit_expression_mut(update);
            }
            if let Some(break_hook) = break_hook {
                visitor.visit_expression_mut(break_hook);
            }
            for statement in body {
                visitor.visit_statement_mut(statement);
            }
        }
        Statement::While {
            condition,
            break_hook,
            body,
            ..
        }
        | Statement::DoWhile {
            condition,
            break_hook,
            body,
            ..
        } => {
            visitor.visit_expression_mut(condition);
            if let Some(break_hook) = break_hook {
                visitor.visit_expression_mut(break_hook);
            }
            for statement in body {
                visitor.visit_statement_mut(statement);
            }
        }
    }
}

pub fn walk_switch_case<V: Visitor + ?Sized>(visitor: &mut V, switch_case: &SwitchCase) {
    if let Some(test) = &switch_case.test {
        visitor.visit_expression(test);
    }
    for statement in &switch_case.body {
        visitor.visit_statement(statement);
    }
}

pub fn walk_switch_case_mut<V: VisitorMut + ?Sized>(visitor: &mut V, switch_case: &mut SwitchCase) {
    if let Some(test) = &mut switch_case.test {
        visitor.visit_expression_mut(test);
    }
    for statement in &mut switch_case.body {
        visitor.visit_statement_mut(statement);
    }
}

pub fn walk_expression<V: Visitor + ?Sized>(visitor: &mut V, expression: &Expression) {
    match expression {
        Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined
        | Expression::NewTarget
        | Expression::Identifier(_)
        | Expression::This
        | Expression::Sent
        | Expression::Update { .. } => {}
        Expression::Array(elements) => {
            for element in elements {
                visitor.visit_array_element(element);
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                visitor.visit_object_entry(entry);
            }
        }
        Expression::Member { object, property } => {
            visitor.visit_expression(object);
            visitor.visit_expression(property);
        }
        Expression::SuperMember { property } => {
            visitor.visit_expression(property);
        }
        Expression::Assign { value, .. } => {
            visitor.visit_expression(value);
        }
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            visitor.visit_expression(object);
            visitor.visit_expression(property);
            visitor.visit_expression(value);
        }
        Expression::AssignSuperMember { property, value } => {
            visitor.visit_expression(property);
            visitor.visit_expression(value);
        }
        Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression)
        | Expression::Unary { expression, .. } => {
            visitor.visit_expression(expression);
        }
        Expression::Binary { left, right, .. } => {
            visitor.visit_expression(left);
            visitor.visit_expression(right);
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            visitor.visit_expression(condition);
            visitor.visit_expression(then_expression);
            visitor.visit_expression(else_expression);
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                visitor.visit_expression(expression);
            }
        }
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            visitor.visit_expression(callee);
            for argument in arguments {
                visitor.visit_call_argument(argument);
            }
        }
    }
}

pub fn walk_expression_mut<V: VisitorMut + ?Sized>(visitor: &mut V, expression: &mut Expression) {
    match expression {
        Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined
        | Expression::NewTarget
        | Expression::Identifier(_)
        | Expression::This
        | Expression::Sent
        | Expression::Update { .. } => {}
        Expression::Array(elements) => {
            for element in elements {
                visitor.visit_array_element_mut(element);
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                visitor.visit_object_entry_mut(entry);
            }
        }
        Expression::Member { object, property } => {
            visitor.visit_expression_mut(object);
            visitor.visit_expression_mut(property);
        }
        Expression::SuperMember { property } => {
            visitor.visit_expression_mut(property);
        }
        Expression::Assign { value, .. } => {
            visitor.visit_expression_mut(value);
        }
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            visitor.visit_expression_mut(object);
            visitor.visit_expression_mut(property);
            visitor.visit_expression_mut(value);
        }
        Expression::AssignSuperMember { property, value } => {
            visitor.visit_expression_mut(property);
            visitor.visit_expression_mut(value);
        }
        Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression)
        | Expression::Unary { expression, .. } => {
            visitor.visit_expression_mut(expression);
        }
        Expression::Binary { left, right, .. } => {
            visitor.visit_expression_mut(left);
            visitor.visit_expression_mut(right);
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            visitor.visit_expression_mut(condition);
            visitor.visit_expression_mut(then_expression);
            visitor.visit_expression_mut(else_expression);
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                visitor.visit_expression_mut(expression);
            }
        }
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            visitor.visit_expression_mut(callee);
            for argument in arguments {
                visitor.visit_call_argument_mut(argument);
            }
        }
    }
}

pub fn walk_array_element<V: Visitor + ?Sized>(visitor: &mut V, element: &ArrayElement) {
    visitor.visit_expression(match element {
        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => expression,
    });
}

pub fn walk_array_element_mut<V: VisitorMut + ?Sized>(visitor: &mut V, element: &mut ArrayElement) {
    visitor.visit_expression_mut(match element {
        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => expression,
    });
}

pub fn walk_object_entry<V: Visitor + ?Sized>(visitor: &mut V, entry: &ObjectEntry) {
    match entry {
        ObjectEntry::Data { key, value } => {
            visitor.visit_expression(key);
            visitor.visit_expression(value);
        }
        ObjectEntry::Getter { key, getter } => {
            visitor.visit_expression(key);
            visitor.visit_expression(getter);
        }
        ObjectEntry::Setter { key, setter } => {
            visitor.visit_expression(key);
            visitor.visit_expression(setter);
        }
        ObjectEntry::Spread(expression) => {
            visitor.visit_expression(expression);
        }
    }
}

pub fn walk_object_entry_mut<V: VisitorMut + ?Sized>(visitor: &mut V, entry: &mut ObjectEntry) {
    match entry {
        ObjectEntry::Data { key, value } => {
            visitor.visit_expression_mut(key);
            visitor.visit_expression_mut(value);
        }
        ObjectEntry::Getter { key, getter } => {
            visitor.visit_expression_mut(key);
            visitor.visit_expression_mut(getter);
        }
        ObjectEntry::Setter { key, setter } => {
            visitor.visit_expression_mut(key);
            visitor.visit_expression_mut(setter);
        }
        ObjectEntry::Spread(expression) => {
            visitor.visit_expression_mut(expression);
        }
    }
}

pub fn walk_call_argument<V: Visitor + ?Sized>(visitor: &mut V, argument: &CallArgument) {
    visitor.visit_expression(argument.expression());
}

pub fn walk_call_argument_mut<V: VisitorMut + ?Sized>(
    visitor: &mut V,
    argument: &mut CallArgument,
) {
    visitor.visit_expression_mut(argument.expression_mut());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::hir::{BinaryOp, FunctionKind};

    #[test]
    fn visitor_walks_nested_program_structure() {
        let program = Program {
            strict: false,
            functions: vec![FunctionDeclaration {
                name: "f".to_string(),
                top_level_binding: None,
                params: vec![Parameter {
                    name: "arg".to_string(),
                    default: Some(Expression::Identifier("x".to_string())),
                    rest: false,
                }],
                body: vec![Statement::Return(Expression::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(Expression::Identifier("arg".to_string())),
                    right: Box::new(Expression::Identifier("y".to_string())),
                })],
                register_global: false,
                kind: FunctionKind::Ordinary,
                self_binding: None,
                mapped_arguments: false,
                strict: false,
                lexical_this: false,
                length: 1,
            }],
            statements: vec![Statement::Expression(Expression::Call {
                callee: Box::new(Expression::Identifier("f".to_string())),
                arguments: vec![CallArgument::Expression(Expression::Identifier(
                    "z".to_string(),
                ))],
            })],
        };

        #[derive(Default)]
        struct IdentifierCollector {
            names: Vec<String>,
        }

        impl Visitor for IdentifierCollector {
            fn visit_expression(&mut self, expression: &Expression) {
                if let Expression::Identifier(name) = expression {
                    self.names.push(name.clone());
                }
                walk_expression(self, expression);
            }
        }

        let mut collector = IdentifierCollector::default();
        collector.visit_program(&program);
        assert_eq!(collector.names, vec!["x", "arg", "y", "f", "z"]);
    }

    #[test]
    fn visitor_mut_rewrites_nested_identifiers() {
        let mut program = Program {
            strict: false,
            functions: vec![],
            statements: vec![Statement::Expression(Expression::Conditional {
                condition: Box::new(Expression::Identifier("x".to_string())),
                then_expression: Box::new(Expression::Array(vec![ArrayElement::Expression(
                    Expression::Identifier("x".to_string()),
                )])),
                else_expression: Box::new(Expression::Object(vec![ObjectEntry::Data {
                    key: Expression::String("value".to_string()),
                    value: Expression::Identifier("x".to_string()),
                }])),
            })],
        };

        struct RenameXToY;

        impl VisitorMut for RenameXToY {
            fn visit_expression_mut(&mut self, expression: &mut Expression) {
                if let Expression::Identifier(name) = expression
                    && name == "x"
                {
                    *name = "y".to_string();
                }
                walk_expression_mut(self, expression);
            }
        }

        RenameXToY.visit_program_mut(&mut program);

        let expected = Program {
            strict: false,
            functions: vec![],
            statements: vec![Statement::Expression(Expression::Conditional {
                condition: Box::new(Expression::Identifier("y".to_string())),
                then_expression: Box::new(Expression::Array(vec![ArrayElement::Expression(
                    Expression::Identifier("y".to_string()),
                )])),
                else_expression: Box::new(Expression::Object(vec![ObjectEntry::Data {
                    key: Expression::String("value".to_string()),
                    value: Expression::Identifier("y".to_string()),
                }])),
            })],
        };

        assert_eq!(program, expected);
    }
}
