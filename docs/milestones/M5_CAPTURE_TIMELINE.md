# M5 — Capture + Timeline

**Status:** PROPOSED  
**Baseline:** 2026-09-28

## Purpose

M5 productizes durable local capture and history/timeline behavior without making AI availability a prerequisite.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/03_MEMORY_MODEL.md`, `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Scope

M5 includes:

- offline Quick Capture Inbox
- durable raw capture
- timeline/history/sources
- optional later extraction proposals

## Required invariants

- Quick Capture must remain durable without an LLM.
- Raw Inbox capture is written before optional semantic extraction.
- Provider outage must not prevent memory capture.
- Canonical writes remain daemon-controlled.
- Timeline/history must preserve source/provenance behavior required by the implemented model.

## Non-goals

M5 does not implement:

- broad universal OCR/PDF/DOCX extraction as an assumed V1 requirement
- Obsidian materialization/write-back from M6/M7
- full AI/MCP integrations from M12
- broad Desktop product UI from M13

## Verification

M5 must verify relevant:

- capture while offline
- capture while AI/provider access is unavailable
- durable raw capture before later processing
- timeline/history/source reconstruction from accepted canonical state
- relevant failure/restart behavior

## Completion gate

M5 is complete only when all scope items and relevant tests pass, required CI is green, Definition of Done is satisfied and human review is complete.

Only then may M6 begin.
