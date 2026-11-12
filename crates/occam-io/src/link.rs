//! Source: original
//! Links between records as the native format writes them: in the readable file each in the one text form a report
//! uses, `edge:41/1`, and in the binary twin as its kind, slot and generation. Either is read back only as an id of the
//! kind the field holds, so a link of the wrong kind fails where it is written.

use occam_report::{EntityKind, EntityRef};
use occam_topo::{Id, Kind};
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// `entity` as an id of kind `K`.
fn typed<K: Kind, E: Error>(entity: EntityRef) -> Result<Id<K>, E> {
    Id::from_entity(entity).ok_or_else(|| E::custom(format!("not a {} id: {}", K::ENTITY.text(), entity.text())))
}

/// A link as either encoding writes it: its text form where the encoding is read by people, else its kind, slot and
/// generation, which are shorter and read faster.
#[derive(Clone, Copy, Debug)]
struct Link(EntityRef);

impl Serialize for Link {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            self.0.serialize(serializer)
        } else {
            (self.0.kind, self.0.slot, self.0.generation).serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Link {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            EntityRef::deserialize(deserializer).map(Self)
        } else {
            let (kind, slot, generation) = <(EntityKind, u32, u32)>::deserialize(deserializer)?;
            Ok(Self(EntityRef::new(kind, slot, generation)))
        }
    }
}

/// A link that is always there.
pub(crate) mod one {
    use occam_topo::{Id, Kind};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::{Link, typed};

    /// Writes `id`.
    #[expect(clippy::trivially_copy_pass_by_ref, reason = "the signature serde's `with` calls")]
    pub(crate) fn serialize<K: Kind, S: Serializer>(id: &Id<K>, serializer: S) -> Result<S::Ok, S::Error> {
        Link(id.entity()).serialize(serializer)
    }

    /// Reads an id of kind `K`.
    pub(crate) fn deserialize<'de, K: Kind, D: Deserializer<'de>>(deserializer: D) -> Result<Id<K>, D::Error> {
        typed(Link::deserialize(deserializer)?.0)
    }
}

/// A link that may be `null`.
pub(crate) mod maybe {
    use occam_topo::{Id, Kind};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::{Link, typed};

    /// Writes `id`, or `null`.
    #[expect(clippy::trivially_copy_pass_by_ref, clippy::ref_option, reason = "the signature serde's `with` calls")]
    pub(crate) fn serialize<K: Kind, S: Serializer>(id: &Option<Id<K>>, serializer: S) -> Result<S::Ok, S::Error> {
        id.map(|id| Link(id.entity())).serialize(serializer)
    }

    /// Reads an id of kind `K`, or `null`.
    pub(crate) fn deserialize<'de, K: Kind, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Id<K>>, D::Error> {
        Option::<Link>::deserialize(deserializer)?.map(|link| typed(link.0)).transpose()
    }
}

/// A use's owner: the loop or the wire it belongs to, which its kind tells apart.
pub(crate) mod owner {
    use occam_report::EntityKind;
    use occam_topo::{Loop, UseOwner, Wire};
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::{Link, typed};

    /// Writes the owner.
    pub(crate) fn serialize<S: Serializer>(owner: &UseOwner, serializer: S) -> Result<S::Ok, S::Error> {
        match owner {
            UseOwner::Loop(l) => Link(l.entity()).serialize(serializer),
            UseOwner::Wire(w) => Link(w.entity()).serialize(serializer),
        }
    }

    /// Reads a loop or a wire.
    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<UseOwner, D::Error> {
        let entity = Link::deserialize(deserializer)?.0;
        match entity.kind {
            EntityKind::Loop => typed::<Loop, D::Error>(entity).map(UseOwner::Loop),
            EntityKind::Wire => typed::<Wire, D::Error>(entity).map(UseOwner::Wire),
            _ => Err(D::Error::custom(format!("not a loop or wire id: {}", entity.text()))),
        }
    }
}

#[cfg(test)]
mod tests {
    use occam_report::{EntityKind, EntityRef};
    use occam_topo::{CurveGeom, Id, Loop, UseOwner};
    use serde::{Deserialize, Serialize};

    use super::{Link, maybe, one, owner};

    #[derive(Debug, Serialize, Deserialize)]
    struct Links {
        #[serde(with = "one")]
        curve: Id<CurveGeom>,
        #[serde(with = "maybe")]
        next: Option<Id<CurveGeom>>,
        #[serde(with = "owner")]
        owner: UseOwner,
    }

    /// `links`, each written by `entity` in the binary twin's form.
    fn binary(links: [EntityRef; 3]) -> Vec<u8> {
        let [curve, next, owner] = links.map(Link);
        postcard::to_extend(&(curve, Some(next), owner), Vec::new()).unwrap()
    }

    #[test]
    fn a_binary_link_reads_back_only_as_an_id_of_the_kind_its_field_holds() {
        let curve = EntityRef::new(EntityKind::Curve, 4, 2);
        let next = EntityRef::new(EntityKind::Curve, 0, 7);
        let a_loop = EntityRef::new(EntityKind::Loop, 3, 1);
        let read = |links| postcard::from_bytes::<Links>(&binary(links));
        let links = read([curve, next, a_loop]).unwrap();
        assert_eq!((links.curve.entity(), links.next.map(Id::entity)), (curve, Some(next)));
        assert!(matches!(links.owner, UseOwner::Loop(l) if l == Id::<Loop>::from_entity(a_loop).unwrap()));
        let written = postcard::to_extend(&links, Vec::new()).unwrap();
        assert_eq!(written, binary([curve, next, a_loop]), "written as it is read");
        let parts = postcard::to_extend(&(EntityKind::Curve, 4_u32, 2_u32), Vec::new()).unwrap();
        assert_eq!(postcard::to_extend(&Link(curve), Vec::new()).unwrap(), parts, "its kind, slot and generation");
        let surface = EntityRef::new(EntityKind::Surface, 4, 2);
        assert!(read([surface, next, a_loop]).is_err(), "a surface where a curve is linked");
        assert!(read([curve, surface, a_loop]).is_err(), "a surface where a curve may be linked");
        assert!(read([curve, next, curve]).is_err(), "a curve where a loop or a wire is linked");
        assert!(read([EntityRef::new(EntityKind::Curve, 4, 0), next, a_loop]).is_err(), "a generation of 0");
    }
}
