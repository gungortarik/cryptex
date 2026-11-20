//! Independent implementation for the signed, encrypted M11 archive trial.
//! The executable accepts only the disposable credentials supplied by the test.
use openpgp::parse::Parse;
use openpgp::policy::StandardPolicy;
use openpgp::serialize::stream::{Encryptor, LiteralWriter, Message, Signer};
use openpgp::types::AEADAlgorithm;
use std::io;

fn sign_and_encrypt(
    recipient: &openpgp::Cert,
    signer: &openpgp::Cert,
    input: &mut impl io::Read,
    output: &mut (impl io::Write + Send + Sync),
) -> openpgp::Result<()> {
    let policy = StandardPolicy::new();
    let recipient = recipient
        .keys()
        .with_policy(&policy, None)
        .supported()
        .alive()
        .revoked(false)
        .for_storage_encryption()
        .next()
        .ok_or_else(|| io::Error::other("no usable recovery recipient"))?;
    let secret = signer
        .keys()
        .with_policy(&policy, None)
        .supported()
        .alive()
        .revoked(false)
        .for_signing()
        .unencrypted_secret()
        .next()
        .ok_or_else(|| io::Error::other("no usable backup signer"))?;
    let encrypted = Encryptor::for_recipients(Message::new(output), [recipient])
        .aead_algo(AEADAlgorithm::OCB)
        .build()?;
    let signed = Signer::new(encrypted, secret.key().clone().into_keypair()?)?.build()?;
    let mut literal = LiteralWriter::new(signed).build()?;
    io::copy(input, &mut literal)?;
    literal.finalize()
}

fn fixture() -> openpgp::Result<()> {
    let mut paths = std::env::args_os().skip(1);
    let (Some(recipient), Some(signer), None) = (paths.next(), paths.next(), paths.next()) else {
        return Err(io::Error::other("two disposable credential files required").into());
    };
    sign_and_encrypt(
        &openpgp::Cert::from_file(recipient)?,
        &openpgp::Cert::from_file(signer)?,
        &mut io::stdin().lock(),
        &mut io::stdout(),
    )
}

fn main() -> std::process::ExitCode {
    if fixture().is_ok() {
        std::process::ExitCode::SUCCESS
    } else {
        eprintln!("M11 native fixture encryption failed");
        std::process::ExitCode::FAILURE
    }
}
