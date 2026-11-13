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
        /// A list holds an entity that names another owner, or a shell begins from a face of another shell.
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
        /// An end of an edge is further from its vertex than the vertex's certified radius and the band.
        VertexOffEdge = "vertex-off-edge",
        /// A curve in a face's parameters does not run along its edge on the face's surface.
        ParamCurveOffEdge = "param-curve-off-edge",
        /// A loop does not close in its face's parameters, as one that goes round a seam does not.
        LoopOpenInParameters = "loop-open-in-parameters",
        /// A face's loops leave the period of the surface's parameters that the face's sheet names.
        OutsideSheet = "outside-sheet",
        /// A loop's witness is not strictly inside it.
        WitnessNotInside = "witness-not-inside",
        /// A face's other loops are not strictly inside its outer loop, or the outer loop's witness is inside one.
        LoopsNotNested = "loops-not-nested",
        /// An edge strays from the surface of a face that uses it by more than the band and the near-tangent shift for
        /// its length, `sqrt(2 length band)`.
        EdgeOffFace = "edge-off-face",
        /// An edge's distance from the surface of a face that uses it could not be measured: no point of the surface
        /// was found near one of its points.
        DeviationNotMeasured = "deviation-not-measured",
        /// A loop could not be traced in its face's parameters: no point of the surface was found near one of its
        /// edge's points.
        LoopNotTraced = "loop-not-traced",
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
