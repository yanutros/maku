//! Preuve d'identité du relais.
//!
//! Maku joint le jeton de l'utilisateur à ses appels. Avant cela, il doit savoir à qui il parle : sur un autre
//! réseau, l'adresse du relais peut être celle d'une autre machine, et l'identifiant du serveur, que le relais
//! annonce, est public. Le relais a donc une clé (Ed25519), tirée de la phrase secrète `MAKU_KEY` : il signe le
//! nombre au hasard que Maku lui envoie, et Maku, qui a retenu la clé publique à la première rencontre, sait que
//! c'est bien lui.
//!
//! La clé vient d'une phrase, et non d'un fichier : l'image du relais n'a pas de dossier de données, et la même
//! phrase redonne la même clé après une mise à jour ou une réinstallation.

use anyhow::{anyhow, Result};
use base64::Engine;
use ring::signature::{Ed25519KeyPair, KeyPair};

/// Une phrase plus courte se devinerait à partir de la clé publique.
pub const MIN_PHRASE: usize = 16;
/// Tailles acceptées pour le nombre au hasard de Maku, en octets.
const CHALLENGE: std::ops::RangeInclusive<usize> = 16..=64;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// Graine de la clé : l'empreinte de la phrase.
pub fn seed(phrase: &str) -> [u8; 32] {
    let digest = ring::digest::digest(&ring::digest::SHA256, format!("maku-relay-key-v1\n{phrase}").as_bytes());
    digest.as_ref().try_into().expect("SHA-256 : 32 octets")
}

/// Le nombre au hasard reçu de Maku (base64 d'URL), s'il a une taille acceptable.
pub fn challenge(text: &str) -> Option<Vec<u8>> {
    B64.decode(text).ok().filter(|c| CHALLENGE.contains(&c.len()))
}

/// Ce qui est signé : le type et l'identifiant du serveur, puis le nombre de Maku. L'en-tête fixe empêche de faire
/// signer au relais autre chose qu'une preuve.
pub fn message(kind: &str, id: &str, challenge: &[u8]) -> Vec<u8> {
    let mut m = format!("maku-relay-proof-v1\n{kind}\n{id}\n").into_bytes();
    m.extend_from_slice(challenge);
    m
}

pub struct Signer(Ed25519KeyPair);

impl Signer {
    pub fn new(seed: &[u8; 32]) -> Result<Self> {
        Ed25519KeyPair::from_seed_unchecked(seed).map(Signer).map_err(|_| anyhow!("MAKU_KEY : clé inutilisable"))
    }

    /// Clé publique, en base64 d'URL.
    pub fn public(&self) -> String {
        B64.encode(self.0.public_key().as_ref())
    }

    pub fn sign(&self, kind: &str, id: &str, challenge: &[u8]) -> String {
        B64.encode(self.0.sign(&message(kind, id, challenge)).as_ref())
    }
}
