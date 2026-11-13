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
