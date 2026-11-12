//! Source: original
//! Links between records as the native format writes them: each in the one text form a report uses, `edge:41/1`, and
//! read back only as an id of the kind the field holds, so a link of the wrong kind fails where it is written, with its
//! line and column.

use occam_report::EntityRef;
use occam_topo::{Id, Kind};
use serde::de::Error;

/// `entity` as an id of kind `K`.
fn typed<K: Kind, E: Error>(entity: EntityRef) -> Result<Id<K>, E> {
    Id::from_entity(entity).ok_or_else(|| E::custom(format!("not a {} id: {}", K::ENTITY.text(), entity.text())))
}

/// A link that is always there.
pub(crate) mod one {
    use occam_report::EntityRef;
    use occam_topo::{Id, Kind};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::typed;

    /// Writes `id` in its text form.
    #[expect(clippy::trivially_copy_pass_by_ref, reason = "the signature serde's `with` calls")]
    pub(crate) fn serialize<K: Kind, S: Serializer>(id: &Id<K>, serializer: S) -> Result<S::Ok, S::Error> {
        id.entity().serialize(serializer)
    }

    /// Reads an id of kind `K`.
    pub(crate) fn deserialize<'de, K: Kind, D: Deserializer<'de>>(deserializer: D) -> Result<Id<K>, D::Error> {
        typed(EntityRef::deserialize(deserializer)?)
    }
}

/// A link that may be `null`.
pub(crate) mod maybe {
    use occam_report::EntityRef;
    use occam_topo::{Id, Kind};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::typed;

    /// Writes `id` in its text form, or `null`.
    #[expect(clippy::trivially_copy_pass_by_ref, clippy::ref_option, reason = "the signature serde's `with` calls")]
    pub(crate) fn serialize<K: Kind, S: Serializer>(id: &Option<Id<K>>, serializer: S) -> Result<S::Ok, S::Error> {
        id.map(Id::entity).serialize(serializer)
    }

    /// Reads an id of kind `K`, or `null`.
    pub(crate) fn deserialize<'de, K: Kind, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Id<K>>, D::Error> {
        Option::<EntityRef>::deserialize(deserializer)?.map(typed).transpose()
    }
}

/// A use's owner: the loop or the wire it belongs to, which its kind tells apart.
pub(crate) mod owner {
    use occam_report::{EntityKind, EntityRef};
    use occam_topo::{Loop, UseOwner, Wire};
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::typed;

    /// Writes the owner in its text form.
    pub(crate) fn serialize<S: Serializer>(owner: &UseOwner, serializer: S) -> Result<S::Ok, S::Error> {
        match owner {
            UseOwner::Loop(l) => l.entity().serialize(serializer),
            UseOwner::Wire(w) => w.entity().serialize(serializer),
        }
    }

    /// Reads a loop or a wire.
    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<UseOwner, D::Error> {
        let entity = EntityRef::deserialize(deserializer)?;
        match entity.kind {
            EntityKind::Loop => typed::<Loop, D::Error>(entity).map(UseOwner::Loop),
            EntityKind::Wire => typed::<Wire, D::Error>(entity).map(UseOwner::Wire),
            _ => Err(D::Error::custom(format!("not a loop or wire id: {}", entity.text()))),
        }
    }
}
