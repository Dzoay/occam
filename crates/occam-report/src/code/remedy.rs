//! Source: original
//! The remedies: every kind of fix occam can suggest (L8). A new one is an entry in `text_enum!` below, an arm in each
//! match of `impl RemedyKind`, and a row in the listing test in `code.rs`.

text_enum! {
    /// A kind of fix occam can suggest. The values to apply it with are in the [`crate::Remedy`].
    RemedyKind {
        /// Read or write the report with the schema version in the `version` parameter.
        UseSchema = "use-schema",
        /// Ask for the tolerance in the `tolerance` parameter, which is the model's.
        UseModelTolerance = "use-model-tolerance",
        /// Run on the number of threads in the `threads` parameter.
        UseThreads = "use-threads",
        /// Move the end the report names to the point in the `point` parameter, so that two ends meet.
        MeetAt = "meet-at",
        /// Write the edge the report names as a whole circle about the `centre` parameter, of the `radius` parameter,
        /// facing the `normal` parameter.
        UseCircle = "use-circle",
    }
}

impl RemedyKind {
    /// One sentence on what following it means.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::UseSchema => "Use the schema version given.",
            Self::UseModelTolerance => "Ask for the model's tolerance, or for a coarser one.",
            Self::UseThreads => {
                "Run on the number of threads given. One thread needs none started, so it always works."
            }
            Self::MeetAt => "Move the end the report names to the point given, so that the two ends meet.",
            Self::UseCircle => {
                "Write the edge the report names as a whole circle with the centre, radius and normal given."
            }
        }
    }

    /// The names of the values it carries.
    #[must_use]
    pub const fn parameters(self) -> &'static [&'static str] {
        match self {
            Self::UseSchema => &["version"],
            Self::UseModelTolerance => &["tolerance"],
            Self::UseThreads => &["threads"],
            Self::MeetAt => &["point"],
            Self::UseCircle => &["centre", "radius", "normal"],
        }
    }
}
