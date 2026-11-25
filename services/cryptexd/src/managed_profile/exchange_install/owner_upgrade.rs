//! Prepare only the three derived exchange files for a detached cold generation.
//! The caller owns the stopped enrollment, an independently checked bootstrap,
//! a locked canonical policy and a private staging directory. Never rebind a
//! journal to the staging path: it must reopen at the same enrolled native home.
use super::*;

pub(in crate::managed_profile) fn stage_exchange_upgrade(
    source: &PrivateDirectory,
    mut staging: &PrivateDirectory,
    bootstrap: &[u8],
    policy: &ExchangeConfigPolicy<'_>,
) -> Result<()> {
    use super::super::native_xml::exchange::same_normalized;
    let leaves = [Leaf::Config, Leaf::Binding, Leaf::Journal];
    if source.path() == staging.path() {
        return Err(InstallError::HomeMismatch);
    }
    let raw = source
        .read(Leaf::Journal)?
        .ok_or(InstallError::InvalidJournal)?;
    let mut journal = Journal::decode(&raw)?;
    if journal.native_home != source.home()? {
        return Err(InstallError::HomeMismatch);
    }
    if journal.phase != Phase::Committed
        || journal.encode()? != raw
        || !same_normalized(bootstrap, journal.bootstrap.as_bytes())
            .map_err(|_| InstallError::UnrecognizedState)?
    {
        return Err(InstallError::InvalidJournal);
    }
    let target = policy.prepare_bootstrap(journal.bootstrap.as_bytes())?;
    let old_binding = policy.previous_owner_binding(&target)?;
    let config = source
        .read(Leaf::Config)?
        .ok_or(InstallError::UnrecognizedState)?;
    if target != journal.config.as_bytes()
        || journal.binding.as_bytes() != old_binding
        || source.read(Leaf::Binding)?.as_deref() != Some(&old_binding)
        || !same_normalized(&config, &target).map_err(|_| InstallError::UnrecognizedState)?
    {
        return Err(InstallError::UnrecognizedState);
    }
    let binding = policy.binding_bytes(&target)?;
    policy.verify_binding(&config, &binding)?;
    journal.binding = text(binding.clone())?;
    let committed = journal.encode()?;
    let contents: [&[u8]; 3] = [&config, &binding, &committed];
    // A retry can finish its exact partial output. A different staging
    // owner, an interrupted source install or unexplained bytes must refuse.
    for (leaf, expected) in leaves.into_iter().zip(contents) {
        if source.stage(leaf)?.is_some()
            || staging.stage(leaf)?.is_some_and(|bytes| bytes != expected)
            || staging.read(leaf)?.is_some_and(|bytes| bytes != expected)
        {
            return Err(InstallError::UnrecognizedState);
        }
    }
    for (leaf, expected) in leaves.into_iter().zip(contents) {
        if staging.stage(leaf)?.is_some() || staging.read(leaf)?.as_deref() != Some(expected) {
            staging.replace(leaf, expected)?;
        }
    }
    staging.sync_committed()?;
    verify_committed(&staging, &journal, &committed, policy)
}
