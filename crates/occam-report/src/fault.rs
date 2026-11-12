//! Source: original
//! What kind of failure a report is, why, and where the operation was: three closed lists, chosen where the failure is
//! found, never defaulted from the code (S6: c07 could not tell a repairable input from an unsupported one, and a
//! message's length read as a ratio).

text_enum! {
    /// What kind of failure it is.
    Fault {
        /// The request asked for something that cannot be: a repairable input.
        InvalidInput = "invalid-input",
        /// The request is reasonable and occam cannot do it yet.
        NotImplemented = "not-implemented",
        /// The answer depends on digits the numbers do not have.
        NumericallySensitive = "numerically-sensitive",
        /// Something in occam or its machine went wrong.
        Internal = "internal",
    }
}

text_enum! {
    /// Why it failed, from a closed list rather than free text.
    Reason {
        /// Two surfaces that should meet do not.
        NoSection = "no-section",
        /// An offset lands on the band itself.
        OffsetAtBand = "offset-at-band",
        /// Something is smaller than the model resolves.
        BelowResolution = "below-resolution",
        /// Two surfaces lie on each other.
        SurfacesCoincide = "surfaces-coincide",
        /// No method handles this pair of families.
        PairNotSupported = "pair-not-supported",
        /// A radius is zero or negative.
        RadiusNotPositive = "radius-not-positive",
        /// The operands do not touch.
        OperandsDisjoint = "operands-disjoint",
        /// A solver ran out of the work it was allowed.
        BudgetSpent = "budget-spent",
        /// A parameter is outside where a curve or surface is defined.
        OutsideDomain = "outside-domain",
        /// An edit reserved an id and never gave it a record.
        ReservedNotSet = "reserved-not-set",
        /// A record links to an entity that is not live.
        DeadLink = "dead-link",
        /// A table of the model has no slot left to give.
        TableFull = "table-full",
        /// A loop's uses do not come back to its first.
        LoopNotClosed = "loop-not-closed",
        /// A use does not begin where the use before it ends.
        UsesDoNotMeet = "uses-do-not-meet",
        /// A list of entities runs on without ending, or comes back to an entry other than its first.
        ListBroken = "list-broken",
        /// An entity names an owner whose list does not hold it.
        NotListed = "not-listed",
        /// A list holds an entity that names another owner.
        ListedElsewhere = "listed-elsewhere",
        /// An edge has a number of uses its body's kind does not allow.
        WrongUseCount = "wrong-use-count",
        /// Two uses of an edge run the same way, so the faces on either side of it face the same way.
        UsesNotOpposed = "uses-not-opposed",
        /// An edge is used by faces of more than one shell, or by more than one body.
        EdgeShared = "edge-shared",
        /// A closed shell's counts give no whole genus: `V - E + 2F - L` is odd, or more than 2.
        GenusNotWhole = "genus-not-whole",
        /// A shell's faces are not all joined through their edges.
        ShellNotConnected = "shell-not-connected",
        /// A body holds what its kind does not: a solid or sheet without a shell or with a wire, or a wire body
        /// without a wire or with a shell.
        BodyKindMismatch = "body-kind-mismatch",
    }
}

text_enum! {
    /// Where an operation was when it stopped.
    Stage {
        /// Checking the request.
        Validate = "validate",
        /// Preparing the operands.
        Prepare = "prepare",
        /// Finding which parts may meet.
        Broad = "broad",
        /// Intersecting the pairs that may.
        Pairs = "pairs",
        /// Splitting edges.
        EdgeSplit = "edge-split",
        /// Splitting faces.
        FaceSplit = "face-split",
        /// Deciding what is inside.
        Classify = "classify",
        /// Joining the result.
        Stitch = "stitch",
        /// Checking the result.
        Check = "check",
        /// Rounding or bevelling edges.
        Blend = "blend",
        /// Meshing edges.
        MeshEdges = "mesh-edges",
        /// Meshing faces.
        MeshFaces = "mesh-faces",
        /// Reading a file.
        Load = "load",
        /// Evaluating a curve or a surface.
        Evaluate = "evaluate",
    }
}
