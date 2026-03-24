use super::super::*;

pub(crate) fn lower_binary_operator(operator: SwcBinaryOp) -> Result<BinaryOp> {
    Ok(match operator {
        SwcBinaryOp::Add => BinaryOp::Add,
        SwcBinaryOp::Sub => BinaryOp::Subtract,
        SwcBinaryOp::Mul => BinaryOp::Multiply,
        SwcBinaryOp::Div => BinaryOp::Divide,
        SwcBinaryOp::Mod => BinaryOp::Modulo,
        SwcBinaryOp::Exp => BinaryOp::Exponentiate,
        SwcBinaryOp::BitAnd => BinaryOp::BitwiseAnd,
        SwcBinaryOp::BitOr => BinaryOp::BitwiseOr,
        SwcBinaryOp::BitXor => BinaryOp::BitwiseXor,
        SwcBinaryOp::LShift => BinaryOp::LeftShift,
        SwcBinaryOp::RShift => BinaryOp::RightShift,
        SwcBinaryOp::ZeroFillRShift => BinaryOp::UnsignedRightShift,
        SwcBinaryOp::In => BinaryOp::In,
        SwcBinaryOp::InstanceOf => BinaryOp::InstanceOf,
        SwcBinaryOp::EqEq => BinaryOp::LooseEqual,
        SwcBinaryOp::NotEq => BinaryOp::LooseNotEqual,
        SwcBinaryOp::EqEqEq => BinaryOp::Equal,
        SwcBinaryOp::NotEqEq => BinaryOp::NotEqual,
        SwcBinaryOp::Lt => BinaryOp::LessThan,
        SwcBinaryOp::LtEq => BinaryOp::LessThanOrEqual,
        SwcBinaryOp::Gt => BinaryOp::GreaterThan,
        SwcBinaryOp::GtEq => BinaryOp::GreaterThanOrEqual,
        SwcBinaryOp::LogicalAnd => BinaryOp::LogicalAnd,
        SwcBinaryOp::LogicalOr => BinaryOp::LogicalOr,
        SwcBinaryOp::NullishCoalescing => BinaryOp::NullishCoalescing,
    })
}

pub(crate) fn lower_unary_operator(operator: SwcUnaryOp) -> Result<UnaryOp> {
    Ok(match operator {
        SwcUnaryOp::Minus => UnaryOp::Negate,
        SwcUnaryOp::Plus => UnaryOp::Plus,
        SwcUnaryOp::Bang => UnaryOp::Not,
        SwcUnaryOp::Tilde => UnaryOp::BitwiseNot,
        SwcUnaryOp::TypeOf => UnaryOp::TypeOf,
        SwcUnaryOp::Void => UnaryOp::Void,
        SwcUnaryOp::Delete => UnaryOp::Delete,
    })
}

pub(crate) fn lower_update_operator(operator: SwcUpdateOp) -> UpdateOp {
    match operator {
        SwcUpdateOp::PlusPlus => UpdateOp::Increment,
        SwcUpdateOp::MinusMinus => UpdateOp::Decrement,
    }
}

pub(crate) fn lower_function_kind(is_generator: bool, is_async: bool) -> FunctionKind {
    FunctionKind::from_flags(is_generator, is_async)
}
