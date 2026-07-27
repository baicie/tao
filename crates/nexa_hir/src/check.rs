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
const UNKNOWN_MEMBER: DiagnosticCode = DiagnosticCode::new("E2005");
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
    expression_types: HashMap<SourceSpan, Type>,
    name_resolutions: HashMap<SourceSpan, NameResolution>,
    functions: Vec<FunctionFacts>,
}

impl TypedProgram {
    /// Returns the validated underlying HIR program.
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// Returns the resolved type for a value expression at `span`.
    #[must_use]
    pub fn expression_type(&self, span: SourceSpan) -> Option<&Type> {
        self.expression_types.get(&span)
    }

    /// Returns the resolved declaration or builtin for a name token at `span`.
    #[must_use]
    pub fn name_resolution(&self, span: SourceSpan) -> Option<NameResolution> {
        self.name_resolutions.get(&span).copied()
    }

    /// Iterates over all resolved value-expression types.
    pub fn expression_types(&self) -> impl Iterator<Item = (SourceSpan, &Type)> {
        self.expression_types.iter().map(|(span, ty)| (*span, ty))
    }

    /// Iterates over all resolved source-name targets.
    pub fn name_resolutions(&self) -> impl Iterator<Item = (SourceSpan, NameResolution)> + '_ {
        self.name_resolutions
            .iter()
            .map(|(span, resolution)| (*span, *resolution))
    }

    /// Returns local-slot facts for a resolved source function.
    #[must_use]
    pub fn function_facts(&self, function: FunctionId) -> Option<&FunctionFacts> {
        self.functions.get(function.index())
    }
}

/// A stable source-order identifier for a local slot within one function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(usize);

impl LocalId {
    /// Creates a local identifier from its zero-based slot index.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the zero-based slot index within the containing function.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A stable source-order identifier for a function in one program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(usize);

impl FunctionId {
    /// Creates a function identifier from its zero-based source index.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the zero-based source index within the program.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A compiler-provided operation resolved during semantic analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Builtin {
    /// The scalar `print` function.
    Print,
    /// The fixed array `length` member.
    ArrayLength,
}

/// The semantic target of a source name token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameResolution {
    /// A function-local slot.
    Local(LocalId),
    /// A source function.
    Function(FunctionId),
    /// A compiler-provided operation.
    Builtin(Builtin),
}

/// Slot allocation facts for one validated function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFacts {
    parameters: Vec<LocalId>,
    local_count: usize,
}

impl FunctionFacts {
    /// Returns parameter slots in declaration order.
    #[must_use]
    pub fn parameter_ids(&self) -> &[LocalId] {
        &self.parameters
    }

    /// Returns the number of parameter and local slots used by the function.
    #[must_use]
    pub const fn local_count(&self) -> usize {
        self.local_count
    }
}

#[derive(Default)]
struct FactBuilder {
    expression_types: HashMap<SourceSpan, Type>,
    name_resolutions: HashMap<SourceSpan, NameResolution>,
    functions: Vec<FunctionFacts>,
}

impl FactBuilder {
    fn record_expression(&mut self, span: SourceSpan, ty: Type) {
        let _ = self.expression_types.insert(span, ty);
    }

    fn record_name(&mut self, span: SourceSpan, resolution: NameResolution) {
        let _ = self.name_resolutions.insert(span, resolution);
    }
}

/// Resolves names and validates static semantics for a lowered program.
#[must_use]
pub fn type_check(program: &Program) -> Analysis {
    let mut diagnostics = Vec::new();
    let mut facts = FactBuilder::default();
    let functions = collect_function_signatures(program, &mut diagnostics, &mut facts);

    for function in &program.functions {
        check_function(function, &functions, &mut diagnostics, &mut facts);
    }

    diagnostics.sort_by_key(diagnostic_position);
    let typed = diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity() != nexa_diagnostics::Severity::Error)
        .then(|| TypedProgram {
            program: program.clone(),
            expression_types: facts.expression_types,
            name_resolutions: facts.name_resolutions,
            functions: facts.functions,
        });

    Analysis { typed, diagnostics }
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    parameters: Vec<Type>,
    return_type: Type,
    declaration: Option<SourceSpan>,
    resolution: NameResolution,
}

fn collect_function_signatures(
    program: &Program,
    diagnostics: &mut Vec<Diagnostic>,
    facts: &mut FactBuilder,
) -> HashMap<String, FunctionSignature> {
    let mut functions = HashMap::new();
    functions.insert(
        "print".to_owned(),
        FunctionSignature {
            parameters: vec![Type::Int],
            return_type: Type::Unit,
            declaration: None,
            resolution: NameResolution::Builtin(Builtin::Print),
        },
    );

    for (index, function) in program.functions.iter().enumerate() {
        let function_id = FunctionId::new(index);
        facts.record_name(function.name.span, NameResolution::Function(function_id));
        let signature = FunctionSignature {
            parameters: function
                .parameters
                .iter()
                .map(|parameter| parameter.ty.kind.clone())
                .collect(),
            return_type: function.return_type.kind.clone(),
            declaration: Some(function.name.span),
            resolution: NameResolution::Function(function_id),
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
    facts: &mut FactBuilder,
) {
    let mut checker = FunctionChecker {
        functions,
        diagnostics,
        facts,
        scopes: vec![HashMap::new()],
        return_type: function.return_type.kind.clone(),
        loop_depth: 0,
        parameters: Vec::new(),
        next_local: 0,
    };

    for parameter in &function.parameters {
        if let Some(local) = checker.bind(&parameter.name, Some(parameter.ty.kind.clone()), false) {
            checker.parameters.push(local);
        }
        checker.check_value_type(&parameter.ty);
    }
    checker.check_value_type(&function.return_type);

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

    let valid_main_parameters = function.parameters.is_empty()
        || matches!(
            function.parameters.as_slice(),
            [parameter]
                if parameter.ty.kind == Type::Array(Box::new(Type::String))
        );
    if function.name.text == "main"
        && (!valid_main_parameters || function.return_type.kind != Type::Unit)
    {
        checker.error(
            INVALID_RETURN,
            function.name.span,
            "`main` must take no parameters or one `String[]` parameter and return `Unit`",
            "invalid entry-point signature",
        );
    }

    checker.finish();
}

struct FunctionChecker<'a> {
    functions: &'a HashMap<String, FunctionSignature>,
    diagnostics: &'a mut Vec<Diagnostic>,
    facts: &'a mut FactBuilder,
    scopes: Vec<HashMap<String, Binding>>,
    return_type: Type,
    loop_depth: usize,
    parameters: Vec<LocalId>,
    next_local: usize,
}

#[derive(Debug, Clone)]
struct Binding {
    id: LocalId,
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
        let declared_type = annotation.map(|annotation| annotation.kind.clone());
        if let Some(annotation) = annotation {
            self.check_value_type(annotation);
        }
        let initializer_type =
            self.check_expression_with_expected(initializer, declared_type.as_ref());

        if let (Some(expected), Some(actual)) = (&declared_type, &initializer_type) {
            if expected != actual {
                self.type_mismatch(initializer.span(), expected, actual);
            }
        }

        let _ = self.bind(name, declared_type.or(initializer_type), mutable);
    }

    fn check_assignment(&mut self, statement: &AssignmentStatement) {
        let Some(binding) = self.lookup_binding(&statement.target) else {
            let _ = self.check_expression(&statement.value);
            self.error(
                UNDEFINED_NAME,
                statement.target.span,
                format!("undefined value `{}`", statement.target.text),
                "not found in this scope",
            );
            return;
        };
        self.facts
            .record_name(statement.target.span, NameResolution::Local(binding.id));
        let value_type = self.check_expression_with_expected(&statement.value, binding.ty.as_ref());

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

        if let (Some(expected), Some(actual)) = (&binding.ty, &value_type) {
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
        let return_type = self.return_type.clone();
        match (&return_type, statement.value.as_ref()) {
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
                if let Some(actual) = self.check_expression_with_expected(value, Some(expected)) {
                    if &actual != expected {
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
        self.check_expression_with_expected(expression, None)
    }

    fn check_expression_with_expected(
        &mut self,
        expression: &Expression,
        expected: Option<&Type>,
    ) -> Option<Type> {
        let ty = match expression {
            Expression::Integer { .. } => Some(Type::Int),
            Expression::Boolean { .. } => Some(Type::Bool),
            Expression::String { .. } => Some(Type::String),
            Expression::Array { elements, span } => self.check_array(elements, *span, expected),
            Expression::Index {
                collection,
                index,
                span,
            } => self.check_index(collection, index, *span),
            Expression::Member {
                object,
                member,
                span: _,
            } => self.check_member(object, member),
            Expression::Name(name) => match self.lookup_binding(name) {
                Some(binding) => {
                    self.facts
                        .record_name(name.span, NameResolution::Local(binding.id));
                    binding.ty
                }
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
            Expression::Parenthesized { expression, .. } => {
                self.check_expression_with_expected(expression, expected)
            }
        };

        if let Some(ty) = &ty {
            self.facts.record_expression(expression.span(), ty.clone());
        }

        ty
    }

    fn check_array(
        &mut self,
        elements: &[Expression],
        span: SourceSpan,
        expected: Option<&Type>,
    ) -> Option<Type> {
        let contextual_element = match expected {
            Some(Type::Array(element)) => Some(element.as_ref()),
            _ => None,
        };

        if elements.is_empty() {
            let Some(element) = contextual_element else {
                self.error(
                    TYPE_MISMATCH,
                    span,
                    "cannot infer the element type of an empty array",
                    "add an exact array type context",
                );
                return None;
            };
            return (!contains_invalid_array_element(element))
                .then(|| Type::Array(Box::new(element.clone())));
        }

        let mut element_type = contextual_element.cloned();
        for element in elements {
            let actual = self.check_expression_with_expected(element, contextual_element);
            let Some(actual) = actual else {
                continue;
            };
            if actual == Type::Unit {
                self.error(
                    TYPE_MISMATCH,
                    element.span(),
                    "array elements cannot have type `Unit`",
                    "invalid array element",
                );
                continue;
            }

            if let Some(expected) = &element_type {
                if &actual != expected {
                    self.type_mismatch(element.span(), expected, &actual);
                }
            } else {
                element_type = Some(actual);
            }
        }

        element_type.map(|element| Type::Array(Box::new(element)))
    }

    fn check_index(
        &mut self,
        collection: &Expression,
        index: &Expression,
        span: SourceSpan,
    ) -> Option<Type> {
        let collection_type = self.check_expression(collection);
        let index_type = self.check_expression(index);
        if let Some(index_type) = &index_type {
            if index_type != &Type::Int {
                self.type_mismatch(index.span(), &Type::Int, index_type);
            }
        }

        match collection_type {
            Some(Type::Array(element)) => (index_type == Some(Type::Int)).then_some(*element),
            Some(actual) => {
                self.error(
                    TYPE_MISMATCH,
                    span,
                    format!("cannot index a `{actual}` value"),
                    "expected an array value",
                );
                None
            }
            None => None,
        }
    }

    fn check_member(&mut self, object: &Expression, member: &Name) -> Option<Type> {
        match self.check_expression(object) {
            Some(Type::Array(_)) if member.text == "length" => {
                self.facts
                    .record_name(member.span, NameResolution::Builtin(Builtin::ArrayLength));
                Some(Type::Int)
            }
            Some(actual) => {
                self.error(
                    UNKNOWN_MEMBER,
                    member.span,
                    format!("type `{actual}` has no member `{}`", member.text),
                    "unknown member",
                );
                None
            }
            None => None,
        }
    }

    fn check_value_type(&mut self, reference: &TypeReference) {
        if contains_invalid_array_element(&reference.kind) {
            self.error(
                TYPE_MISMATCH,
                reference.span,
                "array elements cannot have type `Unit`",
                "invalid array element type",
            );
        }
    }

    fn check_unary(&mut self, operator: UnaryOperator, expression: &Expression) -> Option<Type> {
        let actual = self.check_expression(expression)?;
        let expected = match operator {
            UnaryOperator::Not => Type::Bool,
            UnaryOperator::Negate => Type::Int,
        };

        if actual != expected {
            self.type_mismatch(expression.span(), &expected, &actual);
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
                if matches!(left_type, Type::Array(_)) && left_type == right_type {
                    self.error(
                        TYPE_MISMATCH,
                        right.span(),
                        "array equality is not defined",
                        "arrays cannot be compared with `===`",
                    );
                    None
                } else if left_type != right_type {
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
            BinaryOperator::Add => {
                if left_type == Type::String && right_type == Type::String {
                    Some(Type::String)
                } else {
                    self.require_binary_ints(left, right, left_type, right_type, Type::Int)
                }
            }
            BinaryOperator::Subtract | BinaryOperator::Multiply | BinaryOperator::Divide => {
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
        if let Some(left_type) = &left_type {
            if left_type != &Type::Bool {
                self.type_mismatch(left.span(), &Type::Bool, left_type);
            }
        }
        if let Some(right_type) = &right_type {
            if right_type != &Type::Bool {
                self.type_mismatch(right.span(), &Type::Bool, right_type);
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
            self.type_mismatch(left.span(), &Type::Int, &left_type);
        }
        if right_type != Type::Int {
            self.type_mismatch(right.span(), &Type::Int, &right_type);
        }

        (left_type == Type::Int && right_type == Type::Int).then_some(result)
    }

    fn check_call(&mut self, callee: &Expression, arguments: &[Expression]) -> Option<Type> {
        let Expression::Name(name) = callee else {
            let _ = self.check_expression(callee);
            for argument in arguments {
                let _ = self.check_expression(argument);
            }
            self.error(
                TYPE_MISMATCH,
                callee.span(),
                "only named functions can be called",
                "not a callable function name",
            );
            return None;
        };

        if let Some(binding) = self.lookup_binding(name) {
            self.facts
                .record_name(name.span, NameResolution::Local(binding.id));
            for argument in arguments {
                let _ = self.check_expression(argument);
            }
            let binding_description = binding
                .ty
                .as_ref()
                .map_or_else(|| "value".to_owned(), |ty| format!("`{ty}` value"));
            self.error(
                TYPE_MISMATCH,
                name.span,
                format!("cannot call a {binding_description}"),
                "not a function",
            );
            return None;
        }

        if name.text == "print" {
            self.facts
                .record_name(name.span, NameResolution::Builtin(Builtin::Print));
            let argument_types = arguments
                .iter()
                .map(|argument| self.check_expression(argument))
                .collect::<Vec<_>>();
            if arguments.len() != 1 {
                self.error(
                    CALL_ARITY,
                    name.span,
                    format!(
                        "function `print` expects 1 argument(s), found {}",
                        arguments.len()
                    ),
                    "incorrect argument count",
                );
            }
            for (argument, actual) in arguments.iter().zip(argument_types) {
                if let Some(actual) = actual {
                    if !matches!(actual, Type::Int | Type::Bool | Type::String) {
                        self.error(
                            TYPE_MISMATCH,
                            argument.span(),
                            format!("`print` cannot display `{actual}`"),
                            "expected `Int`, `Bool`, or `String`",
                        );
                    }
                }
            }
            return Some(Type::Unit);
        }

        let Some(signature) = self.functions.get(&name.text).cloned() else {
            for argument in arguments {
                let _ = self.check_expression(argument);
            }
            self.error(
                UNDEFINED_NAME,
                name.span,
                format!("undefined function `{}`", name.text),
                "not found in this program",
            );
            return None;
        };
        self.facts.record_name(name.span, signature.resolution);

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

        for (argument, expected) in arguments.iter().zip(&signature.parameters) {
            let actual = self.check_expression_with_expected(argument, Some(expected));
            if let Some(actual) = actual {
                if &actual != expected {
                    self.type_mismatch(argument.span(), expected, &actual);
                }
            }
        }
        for argument in arguments.iter().skip(signature.parameters.len()) {
            let _ = self.check_expression(argument);
        }

        Some(signature.return_type)
    }

    fn bind(&mut self, name: &Name, ty: Option<Type>, mutable: bool) -> Option<LocalId> {
        let scope = self.scopes.last_mut()?;

        if let Some(previous) = scope.get(&name.text) {
            let diagnostic =
                Diagnostic::error(DUPLICATE_NAME, format!("duplicate binding `{}`", name.text))
                    .with_label(Label::primary(name.span, "duplicate binding"))
                    .with_label(Label::secondary(previous.span, "first declared here"));
            self.diagnostics.push(diagnostic);
            None
        } else {
            let id = LocalId::new(self.next_local);
            self.next_local += 1;
            scope.insert(
                name.text.clone(),
                Binding {
                    id,
                    ty,
                    span: name.span,
                    mutable,
                },
            );
            self.facts.record_name(name.span, NameResolution::Local(id));
            Some(id)
        }
    }

    fn lookup_binding(&self, name: &Name) -> Option<Binding> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(&name.text).cloned())
    }

    fn type_mismatch(&mut self, span: SourceSpan, expected: &Type, actual: &Type) {
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

    fn finish(self) {
        self.facts.functions.push(FunctionFacts {
            parameters: self.parameters,
            local_count: self.next_local,
        });
    }
}

fn contains_invalid_array_element(ty: &Type) -> bool {
    match ty {
        Type::Array(element) => {
            element.as_ref() == &Type::Unit || contains_invalid_array_element(element)
        }
        Type::Int | Type::Bool | Type::String | Type::Unit => false,
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
