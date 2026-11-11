# M8 macOS launch carrier reassessment

Status: **BOUNDED DESIGN / ISOLATED EXPERIMENT AUTHORIZED; no product activation**.
Date: 2026-11-10. Authority remains the Master Spec and accepted ADRs, especially
ADR-0007's Unix-domain local IPC. This proposal does not raise the minimum OS,
enable Mac owner operations or authorize canonical writes.

## Why a new direction is needed

The [second owner receipt](../../evidence/m8/m8-macos-launch-owner-receipt-2-20261110.md)
has positive public API, certified self/bundle and fork exclusion observations,
but fails all six intended-child handoff cases. Published XNU ordering explains
why the ordinary dup2 file action cannot recover a source already excluded by
close-on-fork. The receipt does not record actual errno, so that explanation is
an inference, not an invented native diagnostic.

Do not keep sending the same package, remove CLOFORK, introduce a creation race,
use private spawn/fileport APIs or weaken the acceptance inventory.

## Recommended bounded next step

Authorize design and a product-separated experiment for **a kernel-sender-
attributed message bootstrap**, using public Mach APIs. Keep ordinary product
Unix IPC unchanged. This is research into a replacement for the failed launch
handoff, not a decision to move product requests to Mach or XPC.

The design must specify and pass source review before an owner package exists:

1. Public endpoint creation/discovery and cleanup, supported SDK/runtime floor,
   and launch order without inheriting a secret descriptor or relying on a
   mutable shared task-registration window. A rendezvous name may be untrusted
   discovery data; knowing it never authenticates a process.
2. Desktop retains the exact Child identity returned by its own spawn. Daemon
   validates its actual live parent. Both verify kernel-supplied sender identity,
   fixed code identifiers, certified organization and expected image binding.
   UID, name, claimed PID or a token supplied by the sender cannot substitute for
   the kernel sender observation and retained process-instance identity.
3. Bidirectional binding and pin lifetime, shutdown/death, same-PID replacement,
   replay and daemon restart. Each restart must establish a fresh bounded session.
4. No audit-token bytes in argv, environment, runtime files or logs; no custom
   cryptography, private API, unsafe post-fork framework code or weakened gates.
5. A closed receipt inventory that preserves actual stage and typed observed
   outcome. Distinguish spawn failure, child refusal, signal, supervisor timeout
   and the child's own deadline. A label for the intended outcome is insufficient.

First verify public declarations and negative/cleanup cases on disposable CI.
Request a single owner package run only after the relevant paths are executable
and reviewed. Prior raw-Mach receipts do not prove this new design: their inherited
registration/token-carrier setup is not accepted launch custody.

Successful startup authentication alone still leaves Unix forwarded-descriptor/
per-frame authorization, bundle replacement consistency and owner-session
close/expiry/cancel/restart acceptance open. Do not enable product mutations until
all applicable gates and the integration decision are accepted.

## Decision boundary

Option A (recommended): proceed with the bounded design and isolated experiment
above, subject to public-API feasibility and review. No production activation.

Option B: retain the report and existing closed Mac gates; pause the replacement
design. M8 remains open; Windows evidence is preserved.

[AGENTS.md](../../AGENTS.md) requires: "If a requested change in tool roles,
workflow, architecture, branch strategy, milestone scope, or integration strategy
is ambiguous, ask the user before changing direction." Moving away from the
failed inherited-descriptor strategy is a material integration choice here.
The owner subsequently selected Option A explicitly in chat. This authorizes only
the bounded design and isolated experiment; product integration remains unapproved.
