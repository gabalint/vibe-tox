use thiserror::Error;

/// PHI-free protocol failures; no variant contains received content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum AuError {
    #[error("invalid AU configuration")]
    Configuration,
    #[error("AU frame exceeds configured limit")]
    FrameTooLong,
    #[error("invalid AU framing")]
    Framing,
    #[error("AU block check character does not match")]
    Checksum,
    #[error("unsupported AU text character")]
    Character,
    #[error("unknown AU message classification")]
    Classification,
    #[error("invalid AU fixed-width field")]
    Field,
    #[error("invalid AU message length")]
    Length,
    #[error("invalid AU block identification")]
    BlockId,
    #[error("AU block terminator does not match block identification")]
    BlockTerminator,
    #[error("AU sample header changed within a blocked message")]
    HeaderConflict,
    #[error("AU block sequence skipped or reordered")]
    BlockSequence,
    #[error("AU retransmission conflicts with the previous block")]
    BlockConflict,
    #[error("AU sample ends before its final block")]
    IncompleteSample,
    #[error("AU message requires too many blocks")]
    TooManyBlocks,
}
