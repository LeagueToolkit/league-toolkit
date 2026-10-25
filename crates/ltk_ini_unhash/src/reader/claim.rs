//! The names that reads assign to keys, and the best names of each key.

use std::collections::HashMap;

use ltk_hash::IniHash;

use super::part::{eq_ignore_case, write, Parts};
use super::section::Sec;
use crate::{Name, Names, Options, Ty};

/// A name that a read assigns to a key of the file.
#[derive(Debug)]
struct Claim {
    section: Sec,
    /// The property name, with its comment prefix.
    name: String,
    commented: bool,
    ty: Option<Ty>,
    /// `true` if a read outside a stale path claims the key.
    client: bool,
}

impl Claim {
    /// Returns the rank of the claim. A lower rank is a better name. A client read ranks before a
    /// stale read. A name without a comment prefix ranks before a name with a comment prefix.
    fn rank(&self) -> u8 {
        u8::from(self.commented) * 2 + u8::from(!self.client)
    }

    /// Returns `true` if `options` keep the claim.
    fn kept(&self, options: Options) -> bool {
        (options.commented || !self.commented)
            && (options.stale || (self.client && !self.commented))
    }
}

/// The names that reads assign to each key of the file.
#[derive(Debug, Default)]
pub(super) struct Claims(HashMap<IniHash, Vec<Claim>>);

impl Claims {
    /// Assigns `section*comment name` to `key`. A read of a name that `key` has makes the name a
    /// client name if `client` is `true`. Allocates the text of the name only for a new name.
    pub(super) fn add(
        &mut self,
        key: IniHash,
        section: Sec,
        comment: &str,
        name: Parts<'_>,
        ty: Option<Ty>,
        client: bool,
    ) {
        let claims = self.0.entry(key).or_default();
        if let Some(claim) = claims
            .iter_mut()
            .find(|claim| claim.section == section && eq_ignore_case(&claim.name, comment, name))
        {
            claim.client |= client;
            return;
        }
        let mut text = comment.as_bytes().to_vec();
        write(name, &mut text);
        claims.push(Claim {
            section,
            name: String::from_utf8_lossy(&text).into_owned(),
            commented: !comment.is_empty(),
            ty,
            client,
        });
    }

    /// Returns the best names of each named key, after `options` removed names. `sections` holds
    /// the name of each section, indexed by [`Sec`].
    pub(super) fn finish(self, sections: &[String], options: Options) -> Names {
        let mut names = Names::new();
        for (key, claims) in self.0 {
            let claims: Vec<Claim> = claims
                .into_iter()
                .filter(|claim| claim.kept(options))
                .collect();
            let Some(best) = claims.iter().map(Claim::rank).min() else {
                continue;
            };
            let best = claims
                .into_iter()
                .filter(|claim| claim.rank() == best)
                .map(|claim| Name {
                    stale: !claim.client || claim.commented,
                    section: sections[claim.section.0].clone(),
                    name: claim.name,
                    ty: claim.ty,
                })
                .collect();
            names.insert(key, best);
        }
        names
    }
}
