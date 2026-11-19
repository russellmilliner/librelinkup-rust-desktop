//! Property-synchronization macros shared across all `cxx-qt` controller bridges.

/// Sets a `QString` property only when the value actually changed, so the QML binding graph
/// is not re-evaluated on every state sync.
macro_rules! sync_str {
    ($self:ident, $setter:ident, $getter:ident, $value:expr) => {{
        let new_value: String = $value;
        if $self.$getter().to_string() != new_value {
            $self.as_mut().$setter(QString::from(new_value.as_str()));
        }
    }};
}

/// Sets a scalar property only when the value actually changed.
macro_rules! sync_val {
    ($self:ident, $setter:ident, $getter:ident, $value:expr) => {{
        let new_value = $value;
        if *$self.$getter() != new_value {
            $self.as_mut().$setter(new_value);
        }
    }};
}
