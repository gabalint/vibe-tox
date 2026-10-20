//! Type-level redaction of protected health information.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Holds PHI so that `{:?}` and `{}` print [`Phi::REDACTED`] instead of the value.
///
/// Reading the value takes an explicit call to [`Phi::expose`]. Serialization is
/// transparent, because DTOs sent to the UI must carry the value; never pass a
/// `Phi` to a log or error message.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Phi<T>(T);

impl<T> Phi<T> {
    /// What `Debug` and `Display` print.
    pub const REDACTED: &'static str = "[PHI redacted]";

    /// Wraps a value.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Borrows the protected value.
    pub fn expose(&self) -> &T {
        &self.0
    }

    /// Unwraps the protected value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> fmt::Debug for Phi<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(Self::REDACTED)
    }
}

impl<T> fmt::Display for Phi<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(Self::REDACTED)
    }
}

/// The TypeScript type is the inner type's.
impl<T: ts_rs::TS> ts_rs::TS for Phi<T> {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(cfg: &ts_rs::Config) -> String {
        T::name(cfg)
    }

    fn inline(cfg: &ts_rs::Config) -> String {
        T::inline(cfg)
    }

    fn visit_dependencies(visitor: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        T::visit_dependencies(visitor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_and_display_redact_the_value() {
        let name = Phi::new(String::from("SYNTHETIC-CANARY"));
        assert_eq!(format!("{name:?}"), Phi::<String>::REDACTED);
        assert_eq!(format!("{name}"), Phi::<String>::REDACTED);
        assert_eq!(format!("{:?}", Some(&name)), "Some([PHI redacted])");
        assert_eq!(name.expose(), "SYNTHETIC-CANARY");
    }

    #[test]
    fn serde_and_typescript_are_transparent() {
        let name = Phi::new(String::from("SYNTHETIC-CANARY"));
        assert_eq!(
            serde_json::to_string(&name).unwrap(),
            "\"SYNTHETIC-CANARY\""
        );
        let back: Phi<String> = serde_json::from_str("\"SYNTHETIC-CANARY\"").unwrap();
        assert_eq!(back, name);
        use ts_rs::TS;
        assert_eq!(Phi::<String>::inline(&ts_rs::Config::default()), "string");
    }
}
