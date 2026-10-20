//! One spelling for closed code sets across serde, TypeScript, SQLite and audit.

/// Declares a closed set of codes stored as TEXT, with one spelling shared by
/// serde, TypeScript (`ts-rs`), SQLite and audit details. The owning migration
/// constrains the column with a matching `CHECK (... IN (...))`. The calling
/// crate needs `serde`, `ts-rs` and `rusqlite`. Reads accept only TEXT holding
/// one of the codes.
///
/// ```
/// lis_core::text_enum! {
///     /// Where a sample stands.
///     SampleState {
///         /// Waiting for receipt.
///         Pending = "pending",
///         /// Received.
///         Received = "received",
///     }
/// }
/// assert_eq!(SampleState::Received.as_str(), "received");
/// ```
#[macro_export]
macro_rules! text_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident = $text:literal,)+ }) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize, ::ts_rs::TS,
        )]
        $(#[$meta])*
        pub enum $name {
            $($(#[$vmeta])* #[serde(rename = $text)] $variant,)+
        }

        impl $name {
            /// The stored, serialized and audit code.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text,)+
                }
            }
        }

        impl ::rusqlite::types::ToSql for $name {
            fn to_sql(&self) -> ::rusqlite::Result<::rusqlite::types::ToSqlOutput<'_>> {
                Ok(::rusqlite::types::ToSqlOutput::from(self.as_str()))
            }
        }

        impl ::rusqlite::types::FromSql for $name {
            fn column_result(
                value: ::rusqlite::types::ValueRef<'_>,
            ) -> ::rusqlite::types::FromSqlResult<Self> {
                match value.as_str()? {
                    $($text => Ok(Self::$variant),)+
                    _ => Err(::rusqlite::types::FromSqlError::InvalidType),
                }
            }
        }
    };
}
