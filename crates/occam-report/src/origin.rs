//! Source: original
//! Where an entity came from: the step that made it and its part in it, which agents asked about first (S6).

text_enum! {
    /// An entity's part in the step that made it.
    StepRole {
        /// A profile the step swept, extruded or revolved.
        Profile = "profile",
        /// What a boolean cuts from or adds to.
        Target = "target",
        /// What a boolean cuts or adds with.
        Tool = "tool",
        /// An input the step changed, such as a body it filleted.
        Input = "input",
        /// Made by the step from no operand, such as a primitive's faces.
        Made = "made",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_roles_have_stable_text() {
        let texts: Vec<&str> = StepRole::ALL.iter().map(|role| role.text()).collect();
        assert_eq!(texts, ["profile", "target", "tool", "input", "made"]);
        assert_eq!(serde_json::to_string(&StepRole::Tool).unwrap(), r#""tool""#);
    }
}
