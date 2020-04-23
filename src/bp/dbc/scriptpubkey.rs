// LNP/BP Rust Library
// Written in 2019 by
//     Dr. Maxim Orlovsky <orlovsky@pandoracore.com>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the MIT License
// along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

use bitcoin::{
    secp256k1,
    secp256k1::PublicKey,
    blockdata::script::Builder
};

use crate::common::AsSlice;
use crate::bp::scripts::{LockScript, PubkeyScript, LockScriptParseError};
use crate::primitives::commit_verify::{
    CommitmentVerify, Verifiable, EmbedCommittable, EmbeddedCommitment
};
use super::{
    pubkey,
    PubkeyCommitment, LockscriptCommitment, TaprootCommitment, TaprootContainer
};


#[derive(Debug, Display, Error, From)]
#[display_from(Debug)]
pub enum Error {
    //#[derive_from(pubkey::Error)]
    Pubkey(pubkey::Error),

    //#[derive_from(secp256k1::Error)]
    Secp256k1(secp256k1::Error),

    //#[derive_from(LockScriptParseError<bitcoin::PublicKey>)]
    LockScript(LockScriptParseError<bitcoin::PublicKey>),
}

impl From<pubkey::Error> for Error {
    fn from(err: pubkey::Error) -> Self {
        Self::Pubkey(err)
    }
}

impl From<secp256k1::Error> for Error {
    fn from(err: secp256k1::Error) -> Self {
        Self::Secp256k1(err)
    }
}

impl From<LockScriptParseError<bitcoin::PublicKey>> for Error {
    fn from(err: LockScriptParseError<bitcoin::PublicKey>) -> Self {
        Self::LockScript(err)
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
#[non_exhaustive]
pub enum ScriptPubkeyContainer {
    PublicKey(PublicKey),
    PubkeyHash(PublicKey),
    ScriptHash(LockScript),
    TapRoot(TaprootContainer),
    OpReturn(PublicKey),
    OtherScript(PubkeyScript),
}


#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
#[non_exhaustive]
pub enum ScriptPubkeyCommitment {
    PublicKey(PubkeyCommitment),
    LockScript(LockscriptCommitment),
    TapRoot(TaprootCommitment),
}


impl From<ScriptPubkeyContainer> for PubkeyScript {
    fn from(container: ScriptPubkeyContainer) -> Self {
        let script = match container {
            ScriptPubkeyContainer::OtherScript(script) =>
                script.into_inner(),
            ScriptPubkeyContainer::PublicKey(pubkey) =>
                Builder::gen_p2pk(&bitcoin::PublicKey { compressed: false, key: pubkey }).into_script(),
            ScriptPubkeyContainer::PubkeyHash(pubkey) => {
                let keyhash = bitcoin::PublicKey { compressed: false, key: pubkey }.wpubkey_hash();
                Builder::gen_v0_p2wpkh(&keyhash).into_script()
            },
            ScriptPubkeyContainer::ScriptHash(script) =>
                Builder::gen_v0_p2wsh(&script.into_inner().wscript_hash()).into_script(),
            ScriptPubkeyContainer::OpReturn(data) => {
                let keyhash = bitcoin::PublicKey { compressed: false, key: data }.wpubkey_hash();
                Builder::gen_op_return(&keyhash.to_vec()).into_script()
            },
            ScriptPubkeyContainer::TapRoot(taproot_container) => unimplemented!(),
            _ => unimplemented!(),
        };
        script.into()
    }
}


impl<MSG> CommitmentVerify<MSG> for ScriptPubkeyCommitment where
    MSG: EmbedCommittable<Self> + EmbedCommittable<LockscriptCommitment> + AsSlice
{

    #[inline]
    fn reveal_verify(&self, msg: &MSG) -> bool {
        <Self as EmbeddedCommitment<MSG>>::reveal_verify(&self, msg)
    }
}

impl<MSG> EmbeddedCommitment<MSG> for ScriptPubkeyCommitment where
    MSG: EmbedCommittable<Self> + EmbedCommittable<LockscriptCommitment> + AsSlice
{
    type Container = ScriptPubkeyContainer;
    type Error = Error;

    #[inline]
    fn get_original_container(&self) -> Self::Container {
        match self {
            ScriptPubkeyCommitment::PublicKey(cmt) => {
                // TODO: Re-implement by analyzing scriptPubkey content
                let container: PublicKey = EmbeddedCommitment::<MSG>::get_original_container(cmt);
                ScriptPubkeyContainer::PubkeyHash(container)
            },
            ScriptPubkeyCommitment::LockScript(cmt) => {
                // TODO: Re-implement by analyzing scriptPubkey content
                let container: LockScript = EmbeddedCommitment::<MSG>::get_original_container(cmt);
                ScriptPubkeyContainer::ScriptHash(container)
            },
            ScriptPubkeyCommitment::TapRoot(cmt) => {
                let container: TaprootContainer = EmbeddedCommitment::<MSG>::get_original_container(cmt);
                ScriptPubkeyContainer::TapRoot(container)
            },
            _ => unimplemented!()
        }
    }

    fn commit_to(container: Self::Container, msg: &MSG) -> Result<Self, Self::Error> {
        Ok(match container {
            ScriptPubkeyContainer::PublicKey(pubkey) => {
                let cmt = PubkeyCommitment::commit_to(pubkey, msg)?;
                ScriptPubkeyCommitment::PublicKey(cmt)
            },
            ScriptPubkeyContainer::PubkeyHash(pubkey) => {
                let cmt = PubkeyCommitment::commit_to(pubkey, msg)?;
                ScriptPubkeyCommitment::PublicKey(cmt)
            },
            ScriptPubkeyContainer::ScriptHash(script) => {
                let cmt = LockscriptCommitment::commit_to(script, msg)?;
                ScriptPubkeyCommitment::LockScript(cmt)
            },
            ScriptPubkeyContainer::TapRoot(container) => {
                let cmt = TaprootCommitment::commit_to(container, msg)?;
                ScriptPubkeyCommitment::TapRoot(cmt)
            },
            ScriptPubkeyContainer::OpReturn(pubkey) => {
                let cmt = PubkeyCommitment::commit_to(pubkey, msg)?;
                ScriptPubkeyCommitment::PublicKey(cmt)
            }
            ScriptPubkeyContainer::OtherScript(script) => {
                // FIXME: Extract it from the txout
                let script = LockScript::from_inner(script.into_inner());
                let cmt = LockscriptCommitment::commit_to(script, msg)?;
                ScriptPubkeyCommitment::LockScript(cmt)
            },
            _ => unimplemented!()
        })
    }
}

impl<T> Verifiable<ScriptPubkeyCommitment> for T where T: AsSlice { }

impl<T> EmbedCommittable<ScriptPubkeyCommitment> for T where T: AsSlice { }