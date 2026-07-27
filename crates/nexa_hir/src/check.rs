use std::collections::HashMap;

use nexa_diagnostics::{Diagnostic, DiagnosticCode, Label};
use nexa_span::SourceSpan;

use crate::{
    AssignmentStatement, BinaryOperator, Block, ConstDeclaration, Expression, Function,
    IfStatement, LetDeclaration, Name, Program, ReturnStatement, Statement, Type, TypeReference,
    UnaryOperator, WhileStatement,
};

const UNDEFINED_NAME: DiagnosticCode = DiagnosticCode::new("E2001");
const DUPLICATE_NAME: DiagnosticCode = DiagnosticCode::new("E2002");
const CALL_ARITY: DiagnosticCode = DiagnosticCode::new("E2003");
const IMMUTABLE_ASSIGNMENT: DiagnosticCode = DiagnosticCode::new("E2004");
const TYPE_MISMATCH: DiagnosticCode = DiagnosticCode::new("E3001");
const NON_BOOLEAN_CONDITION: DiagnosticCode = DiagnosticCode::new("E3002");
const INVALID_RETURN: DiagnosticCode = DiagnosticCode::new("E3003");
const INVALID_LOOP_CONTROL: DiagnosticCode = DiagnosticCode::new("E3004");

/// The semantic result of type checking a lowered program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    typed: Option<TypedProgram>,
    diagnostics: Vec<Diagnostic>,
}

impl Analysis {
    /// Returns the validated typed HIR when no error diagnostics were produced.
    #[must_use]
    pub fn typed(&self) -> Option<&TypedProgram> {
        self.typed.as_ref()
    }

    /// Returns all semantic diagnostics in source order.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Returns true when semantic analysis produced no error diagnostics.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.typed.is_some()
    }
}

/// A HIR program proven to satisfy the Language Core static type rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedProgram {
    program: Program,
}

impl TypedProgram {
    /// Returns the validated underlying HIR program.
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }
}

/// Resolves names and validates static semantics for a lowered program.
#[must_use]
pub fn type_check(program: &Program) -> Analysis {
    let mut diagnostics = Vec::new();
    let functions = collect_function_signatures(program, &mut diagnostics);

    for function in &program.functions {
        check_function(function, &functions, &mut diagnostics);
    }

    diagnostics.sort_by_key(diagnostic_position);
    let typed = diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity() != nexa_diagnostics::Severity::Error)
        .then(|| TypedProgram {
            program: program.clone(),
        });

    Analysis { typed, diagnostics }
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    parameters: Vec<Type>,
    return_type: Type,
    declaration: Option<SourceSpan>,
}

fn collect_function_signatures(
    program: &Program,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<String, FunctionSignature> {
    let mut functions = HashMap::new();
    functions.insert(
        "print".to_owned(),
        FunctionSignature {
            parameters: vec![Type::Int],
            return_type: Type::Unit,
            declaration: None,
        },
    );

    for function in &program.functions {
        let signature = FunctionSignature {
            parameters: function
                .parameters
                .iter()
                .map(|parameter| parameter.ty.kind)
                .collect(),
            return_type: function.return_type.kind,
            declaration: Some(function.name.span),
        };

        if let Some(previous) = functions.get(&function.name.text) {
            let mut diagnostic = Diagnostic::error(
                DUPLICATE_NAME,
                format!("duplicate function `{}`", function.name.text),
            )
            .with_label(Label::primary(function.name.span, "duplicate function"));
            if let Some(previous) = previous.declaration {
                diagnostic =
                    diagnostic.with_label(Label::secondary(previous, "first declared here"));
            }
            diagnostics.push(diagnostic);
        } else {
            functions.insert(function.name.text.clone(), signature);
        }
    }

    functions
}

fn check_function(
    function: &Function,
    functions: &HashMap<String, FunctionSignature>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut checker = FunctionChecker {
        functions,
        diagnostics,
        scopes: vec![HashMap::new()],
        return_type: function.return_type.kind,
        loop_depth: 0,
    };

    for parameter in &function.parameters {
        checker.bind(&parameter.name, Some(parameter.ty.kind), false);
    }

    let always_returns = checker.check_block(&function.body, false);
    if function.return_type.kind != Type::Unit && !always_returns {
        checker.error(
            INVALID_RETURN,
            function.return_type.span,
            format!(
                "function `{}` may not return `{}` on every path",
                function.name.text, function.return_type.kind
            ),
            "return required on every path",
        );
    }

    if function.name.text == "main"
        && (!function.parameters.is_empty() || function.return_type.kind != Type::Unit)
    {
        checker.error(
            INVALID_RETURN,
            function.name.span,
            "`main` must take no parameters and return `Unit`",
            "invalid entry-point signature",
        );
    }
}

struct FunctionChecker<'a> {
    functions: &'a HashMap<String, FunctionSignature>,
    diagnostics: &'a mut Vec<Diagnostic>,
    scopes: Vec<HashMap<String, Binding>>,
    return_type: Type,
    loop_depth: usize,
}

#[derive(Debug, Clone, Copy)]
struct Binding {
    ty: Option<Type>,
    span: SourceSpan,
    mutable: bool,
}

impl FunctionChecker<'_> {
    fn check_block(&mut self, block: &Block, creates_scope: bool) -> bool {
        if creates_scope {
            self.scopes.push(HashMap::new());
        }

        let mut always_returns = false;
        for statement in &block.statements {
            always_returns |= self.check_statement(statement);
        }

        if creates_scope {
            let _ = self.scopes.pop();
        }

        always_returns
    }

    fn check_statement(&mut self, statement: &Statement) -> bool {
        match statement {
            Statement::Const(declaration) => {
                self.check_const(declaration);
                false
            }
            Statement::Let(declaration) => {
                self.check_let(declaration);
                false
            }
            Statement::Assignment(statement) => {
                self.check_assignment(statement);
                false
            }
            Statement::If(statement) => self.check_if(statement),
            Statement::While(statement) => {
                self.check_while(statement);
                false
            }
            Statement::Break(statement) => {
                self.check_loop_control(statement.span, "break");
                false
            }
            Statement::Continue(statement) => {
                self.check_loop_control(statement.span, "continue");
                false
            }
            Statement::Return(statement) => {
                self.check_return(statement);
                true
            }
            Statement::Expression(statement) => {
                let _ = self.check_expression(&statement.expression);
                false
            }
        }
    }

    fn check_const(&mut self, declaration: &ConstDeclaration) {
        self.check_binding(
            &declaration.name,
            declaration.annotation.as_ref(),
            &declaration.initializer,
            false,
        );
    }

    fn check_let(&mut self, declaration: &LetDeclaration) {
        self.check_binding(
            &declaration.name,
            declaration.annotation.as_ref(),
            &declaration.initializer,
            true,
        );
    }

    fn check_binding(
        &mut self,
        name: &Name,
        annotation: Option<&TypeReference>,
        initializer: &Expression,
        mutable: bool,
    ) {
        let initializer_type = self.check_expression(initializer);
        let declared_type = annotation.map(|annotation| annotation.kind);

        if let (Some(expected), Some(actual)) = (declared_type, initializer_type) {
            if expected != actual {
                self.type_mismatch(initializer.span(), expected, actual);
            }
        }

        self.bind(name, declared_type.or(initializer_type), mutable);
    }

    fn check_assignment(&mut self, statement: &AssignmentStatement) {
        let value_type = self.check_expression(&statement.value);
        let Some(binding) = self.lookup_binding(&statement.target) else {
            self.error(
                UNDEFINED_NAME,
                statement.target.span,
                format!("undefined value `{}`", statement.target.text),
                "not found in this scope",
            );
            return;
        };

        if !binding.mutable {
            self.diagnostics.push(
                Diagnostic::error(
                    IMMUTABLE_ASSIGNMENT,
                    format!(
                        "cannot assign to immutable binding `{}`",
                        statement.target.text
                    ),
                )
                .with_label(Label::primary(
                    statement.target.span,
                    "immutable assignment",
                ))
                .with_label(Label::secondary(binding.span, "binding declared here")),
            );
        }

        if let (Some(expected), Some(actual)) = (binding.ty, value_type) {
            if actual != expected {
                self.type_mismatch(statement.value.span(), expected, actual);
            }
        }
    }

    fn check_if(&mut self, statement: &IfStatement) -> bool {
        if let Some(actual) = self.check_expression(&statement.condition) {
            if actual != Type::Bool {
                self.error(
                    NON_BOOLEAN_CONDITION,
                    statement.condition.span(),
                    format!("if condition must have type `Bool`, found `{actual}`"),
                    "expected `Bool` condition",
                );
            }
        }

        let then_returns = self.check_block(&statement.then_branch, true);
        let else_returns = statement
            .else_branch
            .as_ref()
            .is_some_and(|branch| self.check_block(branch, true));

        then_returns && else_returns
    }

    fn check_while(&mut self, statement: &WhileStatement) {
        if let Some(actual) = self.check_expression(&statement.condition) {
            if actual != Type::Bool {
                self.error(
                    NON_BOOLEAN_CONDITION,
                    statement.condition.span(),
                    format!("while condition must have type `Bool`, found `{actual}`"),
                    "expected `Bool` condition",
                );
            }
        }

        self.loop_depth += 1;
        let _ = self.check_block(&statement.body, true);
        self.loop_depth -= 1;
    }

    fn check_loop_control(&mut self, span: SourceSpan, keyword: &str) {
        if self.loop_depth == 0 {
            self.error(
                INVALID_LOOP_CONTROL,
                span,
                format!("`{keyword}` is only valid inside a loop"),
                "not inside a loop",
            );
        }
    }

    fn check_return(&mut self, statement: &ReturnStatement) {
        match (self.return_type, statement.value.as_ref()) {
            (Type::Unit, None) => {}
            (Type::Unit, Some(value)) => {
                let _ = self.check_expression(value);
                self.error(
                    INVALID_RETURN,
                    value.span(),
                    "`Unit` functions cannot return a value",
                    "unexpected return value",
                );
            }
            (expected, None) => self.error(
                INVALID_RETURN,
                statement.span,
                format!("expected a `{expected}` return value"),
                "missing return value",
            ),
            (expected, Some(value)) => {
                if let Some(actual) = self.check_expression(value) {
                    if actual != expected {
                        self.error(
                            INVALID_RETURN,
                            value.span(),
                            format!("expected return type `{expected}`, found `{actual}`"),
                            "invalid return value",
                        );
                    }
                }
            }
        }
    }

    fn check_expression(&mut self, expression: &Expression) -> Option<Type> {
        match expression {
            Expression::Integer { .. } => Some(Type::Int),
            Expression::Boolean { .. } => Some(Type::Bool),
            Expression::Name(name) => match self.lookup_binding(name) {
                Some(binding) => binding.ty,
                None => {
                    self.error(
                        UNDEFINED_NAME,
                        name.span,
                        format!("undefined value `{}`", name.text),
                        "not found in this scope",
                    );
                    None
                }
            },
            Expression::Unary {
                operator,
                expression,
                ..
            } => self.check_unary(*operator, expression),
            Expression::Binary {
                operator,
                left,
                right,
                ..
            } => self.check_binary(*operator, left, right),
            Expression::Call {
                callee, arguments, ..
            } => self.check_call(callee, arguments),
            Expression::Parenthesized { expression, .. } => self.check_expression(expression),
        }
    }

    fn check_unary(&mut self, operator: UnaryOperator, expression: &Expression) -> Option<Type> {
        let actual = self.check_expression(expression)?;
        let expected = match operator {
            UnaryOperator::Not => Type::Bool,
            UnaryOperator::Negate => Type::Int,
        };

        if actual != expected {
            self.type_mismatch(expression.span(), expected, actual);
            None
        } else {
            Some(expected)
        }
    }

    fn check_binary(
        &mut self,
        operator: BinaryOperator,
        left: &Expression,
        right: &Expression,
    ) -> Option<Type> {
        let left_type = self.check_expression(left);
        let right_type = self.check_expression(right);
        if matches!(
            operator,
            BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr
        ) {
            return self.require_binary_bools(left, right, left_type, right_type);
        }

        let (Some(left_type), Some(right_type)) = (left_type, right_type) else {
            return None;
        };

        match operator {
            BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr => {
                self.require_binary_bools(left, right, Some(left_type), Some(right_type))
            }
            BinaryOperator::Equal => {
                if left_type != right_type {
                    self.error(
                        TYPE_MISMATCH,
                        right.span(),
                        format!("cannot compare `{left_type}` with `{right_type}` using `===`"),
                        "operands must have the same type",
                    );
                    None
                } else {
                    Some(Type::Bool)
                }
            }
            BinaryOperator::Less
            | BinaryOperator::LessEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterEqual => {
                self.require_binary_ints(left, right, left_type, right_type, Type::Bool)
            }
            BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide => {
                self.require_binary_ints(left, right, left_type, right_type, Type::Int)
            }
        }
    }

    fn require_binary_bools(
        &mut self,
        left: &Expression,
        right: &Expression,
        left_type: Option<Type>,
        right_type: Option<Type>,
    ) -> Option<Type> {
        if let Some(left_type) = left_type {
            if left_type != Type::Bool {
                self.type_mismatch(left.span(), Type::Bool, left_type);
            }
        }
        if let Some(right_type) = right_type {
            if right_type != Type::Bool {
                self.type_mismatch(right.span(), Type::Bool, right_type);
            }
        }

        (left_type == Some(Type::Bool) && right_type == Some(Type::Bool)).then_some(Type::Bool)
    }

    fn require_binary_ints(
        &mut self,
        left: &Expression,
        right: &Expression,
        left_type: Type,
        right_type: Type,
        result: Type,
    ) -> Option<Type> {
        if left_type != Type::Int {
            self.type_mismatch(left.span(), Type::Int, left_type);
        }
        if right_type != Type::Int {
            self.type_mismatch(right.span(), Type::Int, right_type);
        }

        (left_type == Type::Int && right_type == Type::Int).then_some(result)
    }

    fn check_call(&mut self, callee: &Expression, arguments: &[Expression]) -> Option<Type> {
        let argument_types = arguments
            .iter()
            .map(|argument| self.check_expression(argument))
            .collect::<Vec<_>>();

        let Expression::Name(name) = callee else {
            let _ = self.check_expression(callee);
            self.error(
                TYPE_MISMATCH,
                callee.span(),
                "only named functions can be called",
                "not a callable function name",
            );
            return None;
        };

        if let Some(binding) = self.lookup_binding(name) {
            let binding_description = binding
                .ty
                .map_or_else(|| "value".to_owned(), |ty| format!("`{ty}` value"));
            self.error(
                TYPE_MISMATCH,
                name.span,
                format!("cannot call a {binding_description}"),
                "not a function",
            );
            return None;
        }

        let Some(signature) = self.functions.get(&name.text).cloned() else {
            self.error(
                UNDEFINED_NAME,
                name.span,
                format!("undefined function `{}`", name.text),
                "not found in this program",
            );
            return None;
        };

        if signature.parameters.len() != arguments.len() {
            self.error(
                CALL_ARITY,
                name.span,
                format!(
                    "function `{}` expects {} argument(s), found {}",
                    name.text,
                    signature.parameters.len(),
                    arguments.len()
                ),
                "incorrect argument count",
            );
        }

        for ((argument, actual), expected) in arguments
            .iter()
            .zip(argument_types)
            .zip(signature.parameters.iter().copied())
        {
            if let Some(actual) = actual {
                if actual != expected {
                    self.type_mismatch(argument.span(), expected, actual);
                }
            }
        }

        Some(signature.return_type)
    }

    fn bind(&mut self, name: &Name, ty: Option<Type>, mutable: bool) {
        let Some(scope) = self.scopes.last_mut() else {
            return;
        };

        if let Some(previous) = scope.get(&name.text) {
            let diagnostic =
                Diagnostic::error(DUPLICATE_NAME, format!("duplicate binding `{}`", name.text))
                    .with_label(Label::primary(name.span, "duplicate binding"))
                    .with_label(Label::secondary(previous.span, "first declared here"));
            self.diagnostics.push(diagnostic);
        } else {
            scope.insert(
                name.text.clone(),
                Binding {
                    ty,
                    span: name.span,
                    mutable,
                },
            );
        }
    }

    fn lookup_binding(&self, name: &Name) -> Option<Binding> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(&name.text).copied())
    }

    fn type_mismatch(&mut self, span: SourceSpan, expected: Type, actual: Type) {
        self.error(
            TYPE_MISMATCH,
            span,
            format!("expected `{expected}`, found `{actual}`"),
            "type mismatch",
        );
    }

    fn error(
        &mut self,
        code: DiagnosticCode,
        span: SourceSpan,
        message: impl Into<String>,
        label: impl Into<String>,
    ) {
        self.diagnostics
            .push(Diagnostic::error(code, message).with_label(Label::primary(span, label)));
    }
}

fn diagnostic_position(diagnostic: &Diagnostic) -> (usize, usize) {
    diagnostic
        .labels()
        .first()
        .map_or((usize::MAX, usize::MAX), |label| {
            (label.span().range().start(), label.span().range().end())
        })
}
